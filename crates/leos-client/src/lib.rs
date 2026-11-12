//! # leos-client
//!
//! The on-device core of the Leos wallet app.
//!
//! Everything the app needs that is not a pixel lives here: the API client,
//! the PIN key derivation, the QR reader and the money formatting. The
//! native UI (Kotlin on Android, Swift on iOS) calls into this crate through
//! a single JSON entry point, so the mobile builds need no generated
//! bindings and the security sensitive code lives in one reviewed place.
//!
//! The PIN never leaves the device. It is stretched into a secret with
//! Argon2id and a device bound salt, and only that secret is sent to the
//! server.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod api;
pub mod dto;
pub mod error;
pub mod ops;

pub use api::ApiClient;
pub use error::{ClientError, Result};
pub use ops::Wallet;
