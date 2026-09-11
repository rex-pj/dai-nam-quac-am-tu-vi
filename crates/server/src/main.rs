//! Composition root — **the only place in the web app that knows PostgreSQL exists**.
//!
//! Every other crate sees only traits. Here we pick the concrete implementations and wire
//! them together, and that is all this file does. If the data store ever changes, this is
//! the only file to edit — not out of discipline, but because the other crates **would not
//! compile** if they knew about SeaORM.
//!
//! The server **does not run migrations** at startup. A migration is a deployment step, run
//! by a person, with a record: several replicas booting together would race, and a broken
//! deploy would rewrite the schema before anyone could look. Use `dnqatv db migrate up`.

use std::sync::Arc;

use anyhow::{Context, Result};
use dnqatv_adapter_db::{PgRepository, connect};
use dnqatv_adapter_web::{AppState, SiteMeta, Templates, router};
use dnqatv_app::bridge::Bridge;
use dnqatv_app::service::{BrowseService, DictionaryService};
use dnqatv_config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config: AppConfig = dnqatv_config::load_from_dotenv_and_env(&root.join(".env"))
        .context("loading configuration — copy .env.example to .env and fill in DATABASE_URL")?;

    // ── Infrastructure ───────────────────────────────────────────────────────
    let db = connect(&config.database)
        .await
        .with_context(|| format!("connecting to {}", config.database.url))?;
    db.ping().await.context("checking the connection")?;

    // One `PgRepository` implements several ports; `Arc` lets a single instance play several
    // roles while each call site still sees only the port it needs.
    let repo = Arc::new(PgRepository::new(db));

    // ── Human-curated data ───────────────────────────────────────────────────
    let bridge = Arc::new(
        Bridge::load(&root.join("review")).context("reading the orthography bridge file")?,
    );

    // ── Application ──────────────────────────────────────────────────────────
    let dictionary =
        DictionaryService::new(repo.clone(), repo.clone(), repo.clone(), bridge.clone());
    let browse = BrowseService::new(
        repo.clone(),
        repo.clone(),
        repo.clone(),
        repo.clone(),
        repo.clone(),
    );

    // ── Web ──────────────────────────────────────────────────────────────────
    // The font directory is scanned at startup: with no files present the page simply
    // declares no @font-face and no preload, instead of pointing at nothing and 404-ing.
    let site = Arc::new(SiteMeta::new(
        format!("http://{}", config.http_addr),
        root.join("frontend/fonts"),
    ));
    // Templates are compiled RIGHT HERE: a syntax error must fire at startup, not when a
    // user clicks the one broken page.
    let templates = Arc::new(Templates::load(site.clone()).context("loading templates")?);

    let app = router(AppState {
        dictionary,
        browse,
        templates,
        site,
    });

    let listener = tokio::net::TcpListener::bind(&config.http_addr)
        .await
        .with_context(|| format!("listening on {}", config.http_addr))?;

    println!("Đại Nam Quấc Âm Tự Vị");
    println!("  running at     http://{}", config.http_addr);
    println!("  database       {}", config.database.url);
    println!("  environment    {}", config.env.as_str());
    println!(
        "  orthography bridge: {} pairs, {} unverified",
        bridge.len(),
        bridge.unverified()
    );

    axum::serve(listener, app).await.context("serving HTTP")?;
    Ok(())
}
