#![allow(clippy::expect_used, clippy::panic)]

//! The connection string: masking the password, and rejecting URL shapes that would do silent harm.

use dnqatv_config::{ConfigError, DatabaseUrl, DbScheme, EnvVar};

const REAL: &str = "postgres://postgres:postgres@localhost:5432/dnqa_tu_vi_db";

#[test]
fn keeps_the_real_string_when_asked_directly() {
    let url = DatabaseUrl::parse(REAL).expect("valid");
    assert_eq!(url.expose(), REAL);
    assert_eq!(url.database(), "dnqa_tu_vi_db");
    assert_eq!(url.scheme(), DbScheme::Postgres);
    assert_eq!(url.password(), Some("postgres"));
}

#[test]
fn both_debug_and_display_mask_the_password() {
    let url = DatabaseUrl::parse(REAL).expect("valid");

    // The most important invariant of this type: the two commonest print paths are safe.
    let printed = format!("{url} | {url:?}");
    assert!(
        !printed.contains(":postgres@"),
        "the password leaked: {printed}"
    );
    assert!(printed.contains("***"));
    assert!(printed.contains("localhost:5432/dnqa_tu_vi_db"));
    // The user name is NOT masked — it is needed for diagnosis and is not a secret.
    assert!(printed.contains("postgres:***@"));
}

#[test]
fn a_password_containing_an_at_sign_still_splits_correctly() {
    // The userinfo/host boundary is the LAST '@', not the first.
    let url = DatabaseUrl::parse("postgres://u:p@ss@db.example.com:5432/x").expect("valid");
    assert_eq!(url.password(), Some("p@ss"));
    assert_eq!(url.database(), "x");
    assert!(!url.redacted().contains("p@ss"));
}

#[test]
fn nothing_to_mask_when_there_is_no_userinfo() {
    let url = DatabaseUrl::parse("postgres://localhost/x").expect("valid");
    assert_eq!(url.password(), None);
    assert_eq!(url.redacted(), "postgres://localhost/x");
}

#[test]
fn accepts_both_standard_schemes() {
    for scheme in DbScheme::ALL {
        let raw = format!("{}localhost/x", scheme.prefix());
        let url = DatabaseUrl::parse(&raw).expect("valid");
        assert_eq!(url.scheme(), scheme);
    }
}

#[test]
fn a_missing_database_name_is_an_error_not_a_default() {
    // The real trap: without a database name the driver connects to the database named
    // after the user, and the migration runs in the wrong place with no warning.
    let err = DatabaseUrl::parse("postgres://postgres:postgres@localhost:5432")
        .expect_err("must be rejected");
    assert_eq!(
        err,
        ConfigError::MalformedUrl {
            var: EnvVar::DatabaseUrl,
            missing: "a database name",
        }
    );
}

#[test]
fn rejects_other_schemes() {
    let err = DatabaseUrl::parse("mysql://root:secret@localhost/x").expect_err("must be rejected");
    let text = err.to_string();
    assert!(text.contains("mysql"), "it should say what it got: {text}");
    // Even when rejecting, the password must not reach the error message.
    assert!(
        !text.contains("secret"),
        "the password leaked into the error: {text}"
    );
}

#[test]
fn no_error_variant_carries_the_password() {
    // Sweep the common malformed shapes; the shared invariant: no error string holds the password.
    let broken = [
        "mysql://u:sieu_bi_mat@h/x",
        "postgres://u:sieu_bi_mat@h",
        "postgres://u:sieu_bi_mat@/x",
        "sieu_bi_mat",
    ];
    for raw in broken {
        if let Err(e) = DatabaseUrl::parse(raw) {
            assert!(
                !e.to_string().contains("sieu_bi_mat"),
                "the error for {raw:?} leaked the secret: {e}"
            );
        }
    }
}

#[test]
fn a_query_string_is_not_counted_as_the_database_name() {
    let url = DatabaseUrl::parse("postgres://u:p@h/mydb?sslmode=require").expect("valid");
    assert_eq!(url.database(), "mydb");
}
