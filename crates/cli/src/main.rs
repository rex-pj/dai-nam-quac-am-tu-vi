//! `dnqatv` — the operations tool: configuration and schema.
//!
//! This is a **composition root**: the only place (alongside `server`) that knows where
//! configuration comes from and that PostgreSQL exists. Library crates only ever receive
//! validated values.
//!
//! Why migrations are a separate command rather than a startup step: see `adapter_db::migrate`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use dnqatv_adapter_db::{Db, connect, health, migrate};
use dnqatv_config::{AppConfig, ConfigError, EnvVar, env_var};

/// The local configuration file name and its example — constants, not scattered strings.
const DOTENV: &str = ".env";
const DOTENV_EXAMPLE: &str = ".env.example";

/// The subcommands. An enum so `match` stays exhaustive — adding a command without wiring
/// it up is a compile error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    ConfigExample,
    ConfigShow,
    DbHealth,
    DbMigrateUp,
    DbMigrateDown,
    DbMigrateFresh,
    DbMigrateStatus,
}

impl Command {
    const ALL: [Command; 7] = [
        Self::ConfigExample,
        Self::ConfigShow,
        Self::DbHealth,
        Self::DbMigrateUp,
        Self::DbMigrateDown,
        Self::DbMigrateFresh,
        Self::DbMigrateStatus,
    ];

    const fn words(self) -> &'static [&'static str] {
        match self {
            Self::ConfigExample => &["config", "example"],
            Self::ConfigShow => &["config", "show"],
            Self::DbHealth => &["db", "health"],
            Self::DbMigrateUp => &["db", "migrate", "up"],
            Self::DbMigrateDown => &["db", "migrate", "down"],
            Self::DbMigrateFresh => &["db", "migrate", "fresh"],
            Self::DbMigrateStatus => &["db", "migrate", "status"],
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::ConfigExample => "regenerate .env.example from the EnvVar::ALL catalogue",
            Self::ConfigShow => "print the loaded configuration (password masked)",
            Self::DbHealth => "server version, extensions, migration status",
            Self::DbMigrateUp => "apply the pending migrations",
            Self::DbMigrateDown => "roll back one step",
            Self::DbMigrateFresh => "WIPE EVERYTHING and rebuild — development machines only",
            Self::DbMigrateStatus => "list the migrations and their status",
        }
    }

    /// Whether the command needs a database connection.
    const fn needs_db(self) -> bool {
        !matches!(self, Self::ConfigExample | Self::ConfigShow)
    }

    fn parse(args: &[String]) -> Option<Self> {
        Self::ALL.into_iter().find(|c| {
            let w = c.words();
            args.len() == w.len() && args.iter().zip(w).all(|(a, b)| a == b)
        })
    }
}

fn usage() -> String {
    let mut out = String::from("dnqatv <command>\n\n");
    for c in Command::ALL {
        out.push_str(&format!("  {:<22}{}\n", c.words().join(" "), c.describe()));
    }
    out
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = Command::parse(&args) else {
        bail!("{}", usage());
    };

    let root = workspace_root();

    // `config example` runs even without a .env — it is what helps create one.
    if command == Command::ConfigExample {
        return write_env_example(&root);
    }

    let config = load_config(&root)?;

    match command {
        Command::ConfigExample => unreachable!("handled above"),
        Command::ConfigShow => show_config(&config),
        _ if command.needs_db() => run_db_command(command, &config).await?,
        _ => bail!("command not wired up: {:?}", command),
    }
    Ok(())
}

/// The workspace root — derived from the crate location at compile time, not from the cwd.
///
/// Running `cargo run` from a subdirectory must still find the right `.env`.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn load_config(root: &Path) -> Result<AppConfig> {
    dnqatv_config::load_from_dotenv_and_env(&root.join(DOTENV)).map_err(|e| match e {
        ConfigError::Missing(var) => anyhow::anyhow!(
            "{} is missing. Copy {DOTENV_EXAMPLE} to {DOTENV} and fill it in.",
            var.name()
        ),
        other => anyhow::Error::from(other),
    })
}

fn write_env_example(root: &Path) -> Result<()> {
    let path = root.join(DOTENV_EXAMPLE);
    std::fs::write(&path, env_var::render_example())
        .with_context(|| format!("ghi {}", path.display()))?;
    println!("generated {}", path.display());
    Ok(())
}

fn show_config(config: &AppConfig) {
    // `DatabaseUrl` masks the password in both Display and Debug, so printing it here is
    // safe — safe by type, not by the author remembering.
    println!("environment    : {}", config.env.as_str());
    println!("database       : {}", config.database.url);
    println!("  database name: {}", config.database.url.database());
    println!(
        "  pool         : {}–{} connections",
        config.database.min_connections, config.database.max_connections
    );
    println!("  log SQL      : {}", config.database.sql_logging.as_str());
    println!("web server     : {}", config.http_addr);
    println!("page images    : {}", config.page_image_dir.display());
    println!();
    println!("recognised keys:");
    for var in EnvVar::ALL {
        println!("  {}", var.name());
    }
}

async fn run_db_command(command: Command, config: &AppConfig) -> Result<()> {
    let db = connect(&config.database)
        .await
        .with_context(|| format!("connecting to {}", config.database.url))?;
    db.ping().await.context("checking the connection")?;

    match command {
        Command::DbHealth => print_health(&db).await?,
        Command::DbMigrateUp => {
            migrate::up(&db).await?;
            println!("migration: applied every pending step");
            print_status(&db).await?;
        }
        Command::DbMigrateDown => {
            migrate::down(&db, Some(1)).await?;
            println!("migration: rolled back one step");
            print_status(&db).await?;
        }
        Command::DbMigrateFresh => {
            migrate::fresh(&db).await?;
            println!("migration: rebuilt from scratch");
            print_status(&db).await?;
        }
        Command::DbMigrateStatus => print_status(&db).await?,
        Command::ConfigExample | Command::ConfigShow => {
            bail!("this command does not need a database")
        }
    }

    db.close().await?;
    Ok(())
}

async fn print_status(db: &Db) -> Result<()> {
    for state in migrate::status(db).await? {
        let mark = if state.applied { "applied" } else { "PENDING" };
        println!("  [{mark}] {}", state.name);
    }
    Ok(())
}

async fn print_health(db: &Db) -> Result<()> {
    let h = health::snapshot(db).await?;
    println!("server         : {}", h.server_version);
    println!("extension      : {}", h.extensions.join(", "));
    println!(
        "migration      : {} applied, {} pending",
        h.migrations_applied, h.migrations_pending
    );
    Ok(())
}
