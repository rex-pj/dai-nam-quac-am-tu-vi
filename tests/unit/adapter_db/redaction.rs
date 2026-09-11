#![allow(clippy::expect_used, clippy::panic)]

//! The second line of defence: scrubbing the password out of messages built by lower layers.

use dnqatv_adapter_db::error::scrub;
use dnqatv_config::DatabaseUrl;

#[test]
fn scrubs_the_password_out_of_lower_layer_messages() {
    let url =
        DatabaseUrl::parse("postgres://postgres:sieu_bi_mat@localhost:5432/x").expect("valid");
    let raw = "error connecting to postgres://postgres:sieu_bi_mat@localhost:5432/x: refused";
    let cleaned = scrub(raw, &url);
    assert!(!cleaned.contains("sieu_bi_mat"), "{cleaned}");
    assert!(cleaned.contains("***"));
    // The rest of the message must survive intact — scrubbed, not truncated.
    assert!(cleaned.contains("refused"));
    assert!(cleaned.contains("localhost:5432/x"));
}

#[test]
fn leaves_the_message_alone_when_there_is_no_password() {
    let url = DatabaseUrl::parse("postgres://localhost/x").expect("valid");
    assert_eq!(scrub("any string at all", &url), "any string at all");
}

#[test]
fn scrubs_every_occurrence() {
    let url = DatabaseUrl::parse("postgres://u:pw@h/x").expect("valid");
    assert_eq!(scrub("pw and pw and pw", &url), "*** and *** and ***");
}
