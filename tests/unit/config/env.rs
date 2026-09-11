#![allow(clippy::expect_used, clippy::panic)]

//! Loading configuration: named defaults, errors instead of fallbacks, and a `.env.example` that cannot drift.

use std::time::Duration;

use dnqatv_config::{
    AppConfig, AppEnv, ConfigError, DatabaseConfig, EnvSource, EnvVar, MapEnv, Requirement,
    SqlLogging, dotenv, env_var,
};

fn minimal_env() -> MapEnv {
    MapEnv::new().with(
        EnvVar::DatabaseUrl.name(),
        "postgres://postgres:postgres@localhost:5432/dnqa_tu_vi_db",
    )
}

// ── The key catalogue ────────────────────────────────────────────────────────

#[test]
fn key_names_are_unique() {
    let mut names: Vec<&str> = EnvVar::ALL.iter().map(|v| v.name()).collect();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "two keys share a name");
}

#[test]
fn only_one_key_is_required() {
    // The fewer required keys, the fewer ways to build a broken environment. Only the
    // connection string cannot have a sensible default.
    let required: Vec<&str> = EnvVar::ALL
        .iter()
        .filter(|v| matches!(v.requirement(), Requirement::Required))
        .map(|v| v.name())
        .collect();
    assert_eq!(required, vec!["DATABASE_URL"]);
}

#[test]
fn every_default_parses_through_the_same_path_it_will_travel() {
    // Defaults are declared once, as strings. This test proves those strings pass through
    // the very parser a user value must pass — there is no separate branch for defaults.
    let cfg = DatabaseConfig::from_env(&minimal_env()).expect("the defaults must work");
    assert_eq!(cfg.max_connections, 16);
    assert_eq!(cfg.min_connections, 1);
    assert_eq!(cfg.connect_timeout, Duration::from_secs(8));
    assert_eq!(cfg.acquire_timeout, Duration::from_secs(8));
    assert_eq!(cfg.idle_timeout, Duration::from_secs(300));
    assert_eq!(cfg.sql_logging, SqlLogging::Off);
}

#[test]
fn the_generated_example_file_matches_the_one_on_disk_byte_for_byte() {
    // The anti-drift gate: adding a key without regenerating the file turns this red.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".env.example");
    let on_disk = std::fs::read_to_string(&path).expect(".env.example must exist");
    assert_eq!(
        on_disk.replace("\r\n", "\n"),
        env_var::render_example(),
        "run: cargo run -p dnqatv-cli --bin dnqatv -- config example"
    );
}

#[test]
fn the_example_file_holds_no_real_password() {
    let rendered = env_var::render_example();
    assert!(
        rendered.contains("DATABASE_URL=\n"),
        "required keys are left blank"
    );
    assert!(!rendered.contains("localhost:5432"));
}

// ── Missing and malformed ────────────────────────────────────────────────────

#[test]
fn a_missing_connection_string_is_a_startup_error() {
    let err = DatabaseConfig::from_env(&MapEnv::new()).expect_err("must fail");
    assert_eq!(err, ConfigError::Missing(EnvVar::DatabaseUrl));
}

#[test]
fn a_key_set_to_blank_counts_as_unset() {
    // `DATABASE_URL=` in .env is a typo, not an intention — it must report "missing", not
    // "invalid scheme", because the latter sends people down the wrong path.
    let env = MapEnv::new().with(EnvVar::DatabaseUrl.name(), "   ");
    assert_eq!(
        DatabaseConfig::from_env(&env).expect_err("must fail"),
        ConfigError::Missing(EnvVar::DatabaseUrl)
    );
}

#[test]
fn an_invalid_number_is_an_error_not_a_silent_default() {
    let env = minimal_env().with(EnvVar::DatabaseMaxConnections.name(), "many");
    assert_eq!(
        DatabaseConfig::from_env(&env).expect_err("must fail"),
        ConfigError::NotAnInteger {
            var: EnvVar::DatabaseMaxConnections,
            value: "many".to_owned(),
        }
    );
}

#[test]
fn an_invalid_boolean_is_an_error_not_treated_as_off() {
    let env = minimal_env().with(EnvVar::DatabaseSqlLogging.name(), "yes");
    let err = DatabaseConfig::from_env(&env).expect_err("must fail");
    assert!(err.to_string().contains("true, false"), "{err}");
}

#[test]
fn inverted_pool_bounds_are_an_error() {
    let env = minimal_env()
        .with(EnvVar::DatabaseMaxConnections.name(), "2")
        .with(EnvVar::DatabaseMinConnections.name(), "5");
    assert_eq!(
        DatabaseConfig::from_env(&env).expect_err("must fail"),
        ConfigError::PoolBoundsInverted { min: 5, max: 2 }
    );
}

#[test]
fn an_empty_pool_is_an_error() {
    let env = minimal_env().with(EnvVar::DatabaseMaxConnections.name(), "0");
    assert_eq!(
        DatabaseConfig::from_env(&env).expect_err("must fail"),
        ConfigError::PoolEmpty
    );
}

#[test]
fn the_runtime_environment_is_strict() {
    let env = minimal_env().with(EnvVar::AppEnv.name(), "prod");
    let err = AppConfig::from_env(&env).expect_err("must fail");
    assert!(err.to_string().contains("development, production"), "{err}");

    let env = minimal_env().with(EnvVar::AppEnv.name(), "production");
    assert_eq!(
        AppConfig::from_env(&env).expect("valid").env,
        AppEnv::Production
    );
}

// ── Reading a .env file ──────────────────────────────────────────────────────

#[test]
fn reads_the_common_spellings() {
    let env = dotenv::parse(
        "# a comment\n\
         \n\
         DATABASE_URL=postgres://u:p@h/x\n\
         export APP_ENV=production\n\
         HTTP_ADDR=\"0.0.0.0:80\"\n\
         PAGE_IMAGE_DIR='var/trang'\n\
         a stray line with no equals sign\n",
    );
    assert_eq!(
        env.get(EnvVar::DatabaseUrl.name()),
        Some("postgres://u:p@h/x".to_owned())
    );
    assert_eq!(
        env.get(EnvVar::AppEnv.name()),
        Some("production".to_owned())
    );
    assert_eq!(
        env.get(EnvVar::HttpAddr.name()),
        Some("0.0.0.0:80".to_owned())
    );
    assert_eq!(
        env.get(EnvVar::PageImageDir.name()),
        Some("var/trang".to_owned())
    );
    assert_eq!(env.len(), 4, "the stray line must be skipped, not fatal");
}

#[test]
fn the_real_environment_beats_the_dotenv_file() {
    use dnqatv_config::Layered;
    let real = MapEnv::new().with(EnvVar::AppEnv.name(), "production");
    let file = MapEnv::new()
        .with(EnvVar::AppEnv.name(), "development")
        .with(EnvVar::HttpAddr.name(), "127.0.0.1:9999");
    let layered = Layered(real, file);
    assert_eq!(
        layered.get(EnvVar::AppEnv.name()),
        Some("production".to_owned())
    );
    assert_eq!(
        layered.get(EnvVar::HttpAddr.name()),
        Some("127.0.0.1:9999".to_owned())
    );
}

#[test]
fn a_missing_dotenv_file_is_not_an_error() {
    let env = dotenv::load(std::path::Path::new("does-not-exist.env"));
    assert!(env.is_empty());
}
