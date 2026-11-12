//! Tests for the on-device core.
//!
//! The operations that do not touch the network are exercised here, because
//! they are the ones the phone depends on: the PIN derivation, the QR
//! reader and the money formatting.

use leos_client::Wallet;
use serde_json::Value;

fn wallet() -> Wallet {
    Wallet::new("http://127.0.0.1:1").expect("the core starts")
}

fn data(response: &str) -> Value {
    let json: Value = serde_json::from_str(response).expect("the response is JSON");
    assert_eq!(json["ok"], true, "{response}");
    json["data"].clone()
}

fn error_code(response: &str) -> String {
    let json: Value = serde_json::from_str(response).expect("the response is JSON");
    assert_eq!(json["ok"], false, "{response}");
    json["error"]["code"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn derives_the_same_secret_for_the_same_inputs() {
    let wallet = wallet();

    let first = data(
        &wallet
            .dispatch(r#"{"op":"derive_secret","pin":"4821","device_secret":"device-secret-0123456789ab","salt":"saltsaltsalt"}"#)
            .await,
    );
    let second = data(
        &wallet
            .dispatch(r#"{"op":"derive_secret","pin":"4821","device_secret":"device-secret-0123456789ab","salt":"saltsaltsalt"}"#)
            .await,
    );

    assert_eq!(first["secret"], second["secret"]);
    assert_eq!(first["secret"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn a_different_pin_gives_a_different_secret() {
    let wallet = wallet();

    let first = data(
        &wallet
            .dispatch(r#"{"op":"derive_secret","pin":"4821","device_secret":"device-secret-0123456789ab","salt":"saltsaltsalt"}"#)
            .await,
    );
    let second = data(
        &wallet
            .dispatch(r#"{"op":"derive_secret","pin":"1357","device_secret":"device-secret-0123456789ab","salt":"saltsaltsalt"}"#)
            .await,
    );

    assert_ne!(first["secret"], second["secret"]);
}

#[tokio::test]
async fn checks_the_pin_policy() {
    let wallet = wallet();

    let good = data(
        &wallet
            .dispatch(r#"{"op":"validate_pin","pin":"4821"}"#)
            .await,
    );
    assert_eq!(good["valid"], true);

    let short = data(&wallet.dispatch(r#"{"op":"validate_pin","pin":"12"}"#).await);
    assert_eq!(short["valid"], false);

    let repeated = data(
        &wallet
            .dispatch(r#"{"op":"validate_pin","pin":"1111"}"#)
            .await,
    );
    assert_eq!(repeated["valid"], false);

    let sequential = data(
        &wallet
            .dispatch(r#"{"op":"validate_pin","pin":"1234"}"#)
            .await,
    );
    assert_eq!(sequential["valid"], false);
}

#[tokio::test]
async fn reads_a_merchant_qr_code() {
    let wallet = wallet();

    // A QR Ph payload with a valid checksum.
    let payload = "00020101021126500015com.leos.wallet0108LEOSPHMM0215MERCHANT-0001235204581453039805802PH5918Kasambahay Kitchen6011Mandaluyong63042F97";
    let request = format!(r#"{{"op":"parse_qr","payload":"{payload}"}}"#);

    let parsed = data(&wallet.dispatch(&request).await);
    assert_eq!(parsed["merchant_name"], "Kasambahay Kitchen");
    assert_eq!(parsed["merchant_city"], "Mandaluyong");
    assert_eq!(parsed["merchant_id"], "MERCHANT-000123");
    assert_eq!(parsed["country"], "PH");
    assert_eq!(parsed["domestic"], true);
}

#[tokio::test]
async fn refuses_a_qr_code_with_a_broken_checksum() {
    let wallet = wallet();

    let payload = "00020101021126500015com.leos.wallet0108LEOSPHMM0215MERCHANT-0001235204581453039805802PH5918Kasamblaho Kitchen6011Mandaluyong63042F97";
    let request = format!(r#"{{"op":"parse_qr","payload":"{payload}"}}"#);

    let response = wallet.dispatch(&request).await;
    assert_eq!(error_code(&response), "invalid_request");
}

#[tokio::test]
async fn formats_amounts() {
    let wallet = wallet();

    let formatted = data(
        &wallet
            .dispatch(r#"{"op":"format_amount","amount":"1234.5"}"#)
            .await,
    );
    assert_eq!(formatted["plain"], "1234.50");
    assert_eq!(formatted["formatted"], "LC 1,234.50");
    assert_eq!(formatted["centavos"], 123450);
}

#[tokio::test]
async fn normalizes_phone_numbers() {
    let wallet = wallet();

    let parsed = data(
        &wallet
            .dispatch(r#"{"op":"normalize_phone","phone":"0917 123 4567"}"#)
            .await,
    );
    assert_eq!(parsed["e164"], "+639171234567");
    assert_eq!(parsed["masked"], "+63 917 *** 4567");
    assert_eq!(parsed["mobile"], true);
}

#[tokio::test]
async fn reports_an_unknown_operation() {
    let wallet = wallet();
    let response = wallet.dispatch(r#"{"op":"teleport"}"#).await;
    assert_eq!(error_code(&response), "invalid_request");
}

#[tokio::test]
async fn reports_malformed_json() {
    let wallet = wallet();
    let response = wallet.dispatch("this is not json").await;
    assert_eq!(error_code(&response), "invalid_request");
}
