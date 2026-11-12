//! EMVCo merchant-presented QR codec, using the Philippine QR Ph profile.
//!
//! A merchant QR payload is a TLV byte string: a two digit tag, a two digit
//! length, then the value. The payload ends with tag `63`, whose value is a
//! four character CRC16-CCITT-FALSE checksum over everything from the first
//! byte up to but not including the `63` tag.
//!
//! Leos speaks the QR Ph profile, which differs from plain EMVCo in where the
//! country code, merchant name and city live:
//!
//! | Tag | Meaning (QR Ph)        |
//! |-----|------------------------|
//! | 00  | Payload format         |
//! | 01  | Point of initiation    |
//! | 26  | Merchant account info  |
//! | 52  | Merchant category code |
//! | 53  | Currency (980 = LCO)   |
//! | 54  | Amount                 |
//! | 58  | Country code           |
//! | 59  | Merchant name          |
//! | 60  | Merchant city          |
//! | 61  | Postal code            |
//! | 63  | CRC16 checksum         |
//!
//! Plain EMVCo payloads are also accepted: a name in tag `57` and a city in
//! tag `58` are understood when the QR Ph tags are absent.
//!
//! Unknown tags are preserved verbatim so that acquirer extensions survive a
//! decode/encode round trip.

use serde::{Deserialize, Serialize};

use crate::money::Money;

/// The numeric code of the wallet's own currency, as used in a QR code.
///
/// This is not an ISO 4217 code. The wallet is a simulation, so it settles
/// in a currency that exists only here.
pub const LEOS_CURRENCY: u16 = 980;

/// Country code for the Philippines.
pub const PH_COUNTRY: &str = "PH";

/// The point of initiation method tag value for a static QR.
pub const POI_STATIC: &str = "11";

/// The point of initiation method tag value for a dynamic QR.
pub const POI_DYNAMIC: &str = "12";

/// The merchant account information template used by Leos.
pub const LEOS_ACCOUNT_TEMPLATE: &str = "26";

/// The globally unique identifier Leos publishes for its account template.
pub const LEOS_GUID: &str = "com.leos.wallet";

/// A decoded merchant-presented QR code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantQr {
    /// Payload format indicator, normally `01`.
    pub payload_format: String,
    /// Point of initiation method: `11` static, `12` dynamic.
    pub poi: Option<String>,
    /// Merchant category code.
    pub mcc: Option<String>,
    /// ISO 4217 numeric currency code.
    pub currency: Option<u16>,
    /// Transaction amount, when the QR is amount-fixed.
    pub amount: Option<Money>,
    /// Merchant name.
    pub merchant_name: Option<String>,
    /// Merchant city.
    pub merchant_city: Option<String>,
    /// Merchant postal code.
    pub postal_code: Option<String>,
    /// Merchant tax identifier.
    pub merchant_tax_id: Option<String>,
    /// Country code, `PH` for domestic QR.
    pub country: Option<String>,
    /// The merchant account identifier the payment should be routed to.
    pub account_info: Option<AccountInfo>,
    /// Tags that Leos does not interpret, preserved for round-tripping.
    pub extra: Vec<(String, String)>,
}

/// The account a payment is routed to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    /// Globally unique identifier, e.g. `com.leos.wallet`.
    pub guid: Option<String>,
    /// Bank identifier code.
    pub bic: Option<String>,
    /// Account identifier at the institution.
    pub id: Option<String>,
}

impl MerchantQr {
    /// Builds a QR description for a Leos merchant.
    pub fn leos_merchant(merchant_id: &str, name: &str, city: &str, amount: Option<Money>) -> Self {
        Self {
            payload_format: "01".to_string(),
            poi: Some(POI_STATIC.to_string()),
            mcc: Some("5814".to_string()),
            currency: Some(LEOS_CURRENCY),
            amount,
            merchant_name: Some(name.to_string()),
            merchant_city: Some(city.to_string()),
            postal_code: None,
            merchant_tax_id: None,
            country: Some(PH_COUNTRY.to_string()),
            account_info: Some(AccountInfo {
                guid: Some(LEOS_GUID.to_string()),
                bic: Some("LEOSPHMM".to_string()),
                id: Some(merchant_id.to_string()),
            }),
            extra: Vec::new(),
        }
    }

    /// Decodes a scanned QR payload.
    pub fn from_payload(payload: &str) -> Result<Self, QrError> {
        let payload = payload.trim();
        if payload.is_empty() {
            return Err(QrError::Empty);
        }
        if !payload.is_ascii() {
            return Err(QrError::NotAscii);
        }

        let entries = parse_tlv(payload)?;

        let crc_entry = entries
            .iter()
            .find(|entry| entry.tag == "63")
            .ok_or(QrError::MissingCrc)?;

        // The checksum covers every byte preceding the `63` tag.
        let expected = u16::from_str_radix(&crc_entry.value, 16).map_err(|_| QrError::BadCrc)?;
        let computed = crc16(&payload.as_bytes()[..crc_entry.offset]);
        if computed != expected {
            return Err(QrError::CrcMismatch { expected, computed });
        }

        let mut qr = MerchantQr {
            payload_format: String::new(),
            poi: None,
            mcc: None,
            currency: None,
            amount: None,
            merchant_name: None,
            merchant_city: None,
            postal_code: None,
            merchant_tax_id: None,
            country: None,
            account_info: None,
            extra: Vec::new(),
        };

        let mut name_qrph: Option<String> = None;
        let mut name_emvco: Option<String> = None;
        let mut city_emvco: Option<String> = None;
        let mut account_template: Option<String> = None;

        for entry in &entries {
            let value = &entry.value;
            match entry.tag.as_str() {
                "00" => qr.payload_format = value.clone(),
                "01" => qr.poi = Some(value.clone()),
                "26" | "64" => {
                    if account_template.is_none() {
                        account_template = Some(value.clone());
                    }
                }
                "52" => qr.mcc = Some(value.clone()),
                "53" => qr.currency = value.parse::<u16>().ok(),
                "54" => qr.amount = Money::parse(value).ok(),
                "56" => qr.merchant_tax_id = Some(value.clone()),
                "57" => name_emvco = Some(value.clone()),
                "58" => {
                    // QR Ph puts the country code here; plain EMVCo puts the
                    // city. A two letter uppercase value is a country code.
                    if value.len() == 2 && value.chars().all(|c| c.is_ascii_uppercase()) {
                        qr.country = Some(value.clone());
                    } else {
                        city_emvco = Some(value.clone());
                    }
                }
                "59" => name_qrph = Some(value.clone()),
                "60" => qr.merchant_city = Some(value.clone()),
                "61" => qr.postal_code = Some(value.clone()),
                "63" => {}
                _ => qr.extra.push((entry.tag.clone(), value.clone())),
            }
        }

        qr.merchant_name = name_qrph.or(name_emvco);
        if qr.merchant_city.is_none() {
            qr.merchant_city = city_emvco;
        }

        if let Some(template) = account_template {
            qr.account_info = decode_account_info(&template);
        }

        if qr.payload_format.is_empty() {
            return Err(QrError::MissingPayloadFormat);
        }

        Ok(qr)
    }

    /// Whether the QR names a currency we can settle.
    pub fn is_supported_currency(&self) -> bool {
        match self.currency {
            None => true,
            Some(code) => code == LEOS_CURRENCY,
        }
    }

    /// Whether the QR is a domestic Philippine QR.
    pub fn is_domestic(&self) -> bool {
        self.country.as_deref() == Some(PH_COUNTRY)
    }

    /// The merchant identifier the payment is routed to.
    pub fn merchant_id(&self) -> Option<&str> {
        self.account_info
            .as_ref()
            .and_then(|account| account.id.as_deref())
    }

    /// Builds a Leos merchant QR payload.
    pub fn to_payload(&self) -> String {
        let mut body = String::new();

        push_tag(&mut body, "00", &self.payload_format);
        if let Some(poi) = &self.poi {
            push_tag(&mut body, "01", poi);
        }
        if let Some(account) = &self.account_info {
            let template = encode_account_info(account);
            push_tag(&mut body, LEOS_ACCOUNT_TEMPLATE, &template);
        }
        if let Some(mcc) = &self.mcc {
            push_tag(&mut body, "52", mcc);
        }
        if let Some(currency) = self.currency {
            push_tag(&mut body, "53", &currency.to_string());
        }
        if let Some(amount) = self.amount {
            push_tag(&mut body, "54", &amount.to_string());
        }
        if let Some(tax_id) = &self.merchant_tax_id {
            push_tag(&mut body, "56", tax_id);
        }
        if let Some(country) = &self.country {
            push_tag(&mut body, "58", country);
        }
        if let Some(name) = &self.merchant_name {
            push_tag(&mut body, "59", name);
        }
        if let Some(city) = &self.merchant_city {
            push_tag(&mut body, "60", city);
        }
        if let Some(postal) = &self.postal_code {
            push_tag(&mut body, "61", postal);
        }
        for (tag, value) in &self.extra {
            if tag == "63" {
                continue;
            }
            push_tag(&mut body, tag, value);
        }

        let crc = crc16(body.as_bytes());
        format!("{body}6304{crc:04X}")
    }
}

/// One decoded TLV entry.
struct TlvEntry {
    tag: String,
    value: String,
    /// Byte offset of this entry within the payload.
    offset: usize,
}

/// Splits a payload into TLV entries.
fn parse_tlv(payload: &str) -> Result<Vec<TlvEntry>, QrError> {
    let bytes = payload.as_bytes();
    let mut entries = Vec::new();
    let mut pos = 0usize;

    while pos < bytes.len() {
        if pos + 4 > bytes.len() {
            return Err(QrError::Malformed);
        }
        let tag = &payload[pos..pos + 2];
        if !tag.chars().all(|c| c.is_ascii_digit()) {
            return Err(QrError::Malformed);
        }
        let length: usize = payload[pos + 2..pos + 4]
            .parse()
            .map_err(|_| QrError::Malformed)?;
        let start = pos + 4;
        let end = start.checked_add(length).ok_or(QrError::Malformed)?;
        if end > bytes.len() {
            return Err(QrError::Malformed);
        }
        entries.push(TlvEntry {
            tag: tag.to_string(),
            value: payload[start..end].to_string(),
            offset: pos,
        });
        pos = end;
    }

    Ok(entries)
}

/// Decodes a merchant account information template.
fn decode_account_info(template: &str) -> Option<AccountInfo> {
    let entries = parse_tlv(template).ok()?;
    let mut info = AccountInfo {
        guid: None,
        bic: None,
        id: None,
    };
    for entry in entries {
        match entry.tag.as_str() {
            "00" => info.guid = Some(entry.value),
            "01" => info.bic = Some(entry.value),
            "02" => info.id = Some(entry.value),
            _ => {}
        }
    }
    Some(info)
}

/// Encodes a merchant account information template.
fn encode_account_info(account: &AccountInfo) -> String {
    let mut out = String::new();
    if let Some(guid) = &account.guid {
        push_tag(&mut out, "00", guid);
    }
    if let Some(bic) = &account.bic {
        push_tag(&mut out, "01", bic);
    }
    if let Some(id) = &account.id {
        push_tag(&mut out, "02", id);
    }
    out
}

/// Appends a `tag`, `length`, `value` triple.
fn push_tag(out: &mut String, tag: &str, value: &str) {
    out.push_str(tag);
    out.push_str(&format!("{:02}", value.len()));
    out.push_str(value);
}

/// CRC16-CCITT-FALSE: polynomial `0x1021`, initial value `0xFFFF`, no
/// reflection and no final XOR.
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

/// Failure modes for decoding a merchant QR.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QrError {
    /// The payload was empty.
    #[error("QR payload is empty")]
    Empty,
    /// The payload contained characters outside the QR alphanumeric set.
    #[error("QR payload contains non-ASCII characters")]
    NotAscii,
    /// A tag, length or value was not well formed.
    #[error("QR payload is malformed")]
    Malformed,
    /// The payload had no `63` CRC tag.
    #[error("QR payload is missing its CRC checksum")]
    MissingCrc,
    /// The CRC tag did not contain four hex digits.
    #[error("QR payload has an invalid CRC checksum")]
    BadCrc,
    /// The checksum did not match the payload.
    #[error("QR payload CRC mismatch: expected {expected:04X}, computed {computed:04X}")]
    CrcMismatch {
        /// The checksum carried in the payload.
        expected: u16,
        /// The checksum computed over the payload.
        computed: u16,
    },
    /// The payload had no `00` payload format indicator.
    #[error("QR payload is missing its payload format indicator")]
    MissingPayloadFormat,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sample() -> MerchantQr {
        MerchantQr {
            payload_format: "01".into(),
            poi: Some(POI_STATIC.into()),
            mcc: Some("5814".into()),
            currency: Some(LEOS_CURRENCY),
            amount: Some(Money::from_centavos(12_500)),
            merchant_name: Some("Kasambahay Kitchen".into()),
            merchant_city: Some("Mandaluyong".into()),
            postal_code: None,
            merchant_tax_id: None,
            country: Some(PH_COUNTRY.into()),
            account_info: Some(AccountInfo {
                guid: Some("com.leos.wallet".into()),
                bic: Some("LEOSPHMM".into()),
                id: Some("MERCHANT-000123".into()),
            }),
            extra: Vec::new(),
        }
    }

    #[test]
    fn crc_matches_known_vector() {
        // The check value for CRC16-CCITT-FALSE over "123456789" is 0x29B1.
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }

    #[test]
    fn round_trips_a_merchant_qr() {
        let qr = sample();
        let payload = qr.to_payload();
        let decoded = MerchantQr::from_payload(&payload).expect("decodes");

        assert_eq!(decoded.payload_format, "01");
        assert_eq!(decoded.poi.as_deref(), Some("11"));
        assert_eq!(decoded.merchant_name.as_deref(), Some("Kasambahay Kitchen"));
        assert_eq!(decoded.merchant_city.as_deref(), Some("Mandaluyong"));
        assert_eq!(decoded.mcc.as_deref(), Some("5814"));
        assert_eq!(decoded.currency, Some(LEOS_CURRENCY));
        assert_eq!(decoded.amount, Some(Money::from_centavos(12_500)));
        assert_eq!(decoded.country.as_deref(), Some("PH"));
        assert_eq!(decoded.account_info, qr.account_info);
        assert!(decoded.is_domestic());
        assert!(decoded.is_supported_currency());
        assert_eq!(decoded.merchant_id(), Some("MERCHANT-000123"));
    }

    #[test]
    fn payload_has_a_well_formed_crc_tag() {
        let payload = sample().to_payload();
        let body = &payload.as_bytes()[..payload.len() - 8];
        assert!(payload.ends_with(&format!("6304{:04X}", crc16(body))));
    }

    #[test]
    fn rejects_tampered_payloads() {
        let payload = sample().to_payload();
        let tampered = payload.replace("Kasambahay", "Kasamblaho");
        assert!(matches!(
            MerchantQr::from_payload(&tampered),
            Err(QrError::CrcMismatch { .. })
        ));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(MerchantQr::from_payload("").unwrap_err(), QrError::Empty);
        assert!(MerchantQr::from_payload("not a qr").is_err());
        assert!(MerchantQr::from_payload("0001").is_err());
    }

    #[test]
    fn decodes_a_qr_ph_payload() {
        // A QR Ph payload as printed by a domestic merchant. The checksum is
        // computed over everything preceding the `63` tag.
        let body = concat!(
            "000201",              // 00 payload format
            "010211",              // 01 point of initiation
            "2648",                // 26 merchant account info
            "0015com.leos.wallet", //    00 guid
            "0108LEOSPHMM",        //    01 bic
            "0213MERCHANT-0001",   //    02 account id
            "52045814",            // 52 merchant category code
            "5303980",             // 53 currency
            "5802PH",              // 58 country
            "5910Kasambahay",      // 59 merchant name
            "6011Mandaluyong",     // 60 merchant city
        );
        let payload = format!("{body}6304{:04X}", crc16(body.as_bytes()));

        let decoded = MerchantQr::from_payload(&payload).expect("decodes");
        assert_eq!(decoded.merchant_name.as_deref(), Some("Kasambahay"));
        assert_eq!(decoded.merchant_city.as_deref(), Some("Mandaluyong"));
        assert_eq!(decoded.country.as_deref(), Some("PH"));
        assert_eq!(decoded.currency, Some(LEOS_CURRENCY));
        assert_eq!(decoded.amount, None);
        assert_eq!(decoded.merchant_id(), Some("MERCHANT-0001"));
        assert!(decoded.is_domestic());
    }

    #[test]
    fn decodes_plain_emvco_payloads() {
        // EMVCo puts the city in tag 58 and the name in tag 57.
        let body = concat!(
            "000201",
            "010211",
            "52045814",
            "5303980",
            "5709Coffee Co",
            "5811Makati City",
        );
        let payload = format!("{body}6304{:04X}", crc16(body.as_bytes()));

        let decoded = MerchantQr::from_payload(&payload).expect("decodes");
        assert_eq!(decoded.merchant_name.as_deref(), Some("Coffee Co"));
        assert_eq!(decoded.merchant_city.as_deref(), Some("Makati City"));
        assert_eq!(decoded.country, None);
        assert!(!decoded.is_domestic());
    }

    #[test]
    fn preserves_unknown_tags() {
        let mut qr = sample();
        qr.extra.push(("62".into(), "0912EXTRA DATA".into()));
        let decoded = MerchantQr::from_payload(&qr.to_payload()).expect("decodes");
        assert!(
            decoded
                .extra
                .iter()
                .any(|(tag, value)| tag == "62" && value == "0912EXTRA DATA")
        );
    }
}
