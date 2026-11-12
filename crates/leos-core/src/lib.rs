//! # leos-core
//!
//! Shared domain logic for the Leos wallet. This crate has no I/O, no
//! database and no network dependencies so that the same rules can be
//! enforced by the server, by the on-device Rust core and by the simulators
//! without drift.
//!
//! What lives here:
//!
//! * [`currency`] — the made up currency the wallet holds.
//! * [`money`] — amounts as integer centavos.
//! * [`phone`] — Philippine phone number normalization.
//! * [`qr`] — the EMVCo merchant QR codec used for QR pay.
//! * [`policy`] — PIN rules, transaction limits and fees.
//! * [`domain`] — ledger accounts, postings and transactions.
//! * [`error`] — the single error type with stable codes.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod currency;
pub mod domain;
pub mod error;
pub mod ids;
pub mod money;
pub mod phone;
pub mod pin;
pub mod policy;
pub mod qr;

pub use currency::{Currency, LEOS_CREDITS};
pub use error::{LeosError, Result};
pub use money::Money;
pub use phone::PhoneNumber;
