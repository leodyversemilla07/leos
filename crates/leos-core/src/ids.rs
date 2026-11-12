//! Identifier generation.
//!
//! Leos uses two kinds of identifier:
//!
//! * opaque ids (`usr_...`, `txn_...`) for internal references, and
//! * human readable references (`LEOS-260112-0001`) for things a person has
//!   to read out loud or type in, such as a bank transfer top-up.

use rand::Rng;

/// Generates an opaque identifier with a type prefix, e.g. `usr_3f9a...`.
pub fn new_id(prefix: &str) -> String {
    let bytes: [u8; 16] = rand::rng().random();
    let body = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!("{prefix}_{body}")
}

/// Generates a random uppercase alphanumeric string of the given length.
pub fn random_code(length: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..length)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

/// Generates a numeric one-time passcode of the requested length.
pub fn random_otp(length: usize) -> String {
    let mut rng = rand::rng();
    (0..length)
        .map(|_| char::from_digit(rng.random_range(0..10), 10).unwrap())
        .collect()
}

/// Generates a transaction reference such as `LEOS-20261112-8F3K2Q`.
///
/// The date group shows when the reference was issued, which keeps
/// references roughly sortable and lets an operator eyeball when a top-up
/// was created.
pub fn new_reference() -> String {
    let (year, month, day) = civil_from_days(unix_timestamp().div_euclid(86_400));
    format!("LEOS-{year:04}{month:02}{day:02}-{}", random_code(6))
}

/// Converts a count of days since the Unix epoch into a civil date.
///
/// Uses the standard days-from-civil inverse, which is exact for every date
/// including leap years.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    // Shift the epoch to 0000-03-01 so that leap days land at the end of a
    // 400 year era.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The current Unix timestamp in seconds.
pub fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The current Unix timestamp in milliseconds.
pub fn unix_timestamp_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_prefixed() {
        let ids: HashSet<String> = (0..1_000).map(|_| new_id("usr")).collect();
        assert_eq!(ids.len(), 1_000);
        assert!(ids.iter().all(|id| id.starts_with("usr_")));
    }

    #[test]
    fn otp_is_numeric_with_the_requested_length() {
        let otp = random_otp(6);
        assert_eq!(otp.len(), 6);
        assert!(otp.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn reference_has_the_expected_shape() {
        let reference = new_reference();
        assert!(reference.starts_with("LEOS-"), "{reference}");
        assert_eq!(reference.split('-').count(), 3);
        assert!(reference.len() >= 18, "{reference}");
    }
}
