//! # leos-ffi
//!
//! The bridge between the Leos Rust core and the native mobile interfaces.
//!
//! Two surfaces are exposed over the same implementation:
//!
//! * a plain C ABI for the iOS app, used through a bridging header, and
//! * JNI entry points for the Android app.
//!
//! Both take a JSON request and return a JSON response, so neither mobile
//! build needs generated bindings and neither needs to be rebuilt when an
//! operation is added.
//!
//! Every call is panic safe. A panic in the Rust core is caught at the
//! boundary and turned into an error response rather than unwinding into
//! the app.

#![allow(non_snake_case)]
#![warn(missing_docs)]

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use leos_client::Wallet;

mod jni;

/// The wallets the app has open, keyed by handle.
///
/// A handle is used instead of a raw pointer so that a stale handle from
/// the UI side cannot dereference freed memory.
static WALLETS: LazyLock<Mutex<HashMap<u64, Arc<Wallet>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The next handle to hand out.
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

/// The tokio runtime the network operations run on.
static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("the tokio runtime should build")
});

/// Creates a wallet core pointed at an API.
///
/// Returns a handle, or zero if the address was not usable.
///
/// # Safety
///
/// `base_url` must be a valid null terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn leos_wallet_new(base_url: *const c_char) -> u64 {
    let Some(url) = (unsafe { read_c_string(base_url) }) else {
        return 0;
    };

    let wallet = match Wallet::new(url) {
        Ok(wallet) => wallet,
        Err(_) => return 0,
    };

    let handle = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    let mut wallets = WALLETS.lock().unwrap_or_else(|e| e.into_inner());
    wallets.insert(handle, Arc::new(wallet));
    handle
}

/// Runs an operation and returns the JSON response.
///
/// The returned string is owned by Rust and must be released with
/// [`leos_string_free`].
///
/// # Safety
///
/// `handle` must be a handle returned by [`leos_wallet_new`] that has not
/// been freed, and `request` must be a valid null terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn leos_wallet_call(handle: u64, request: *const c_char) -> *mut c_char {
    let Some(request) = (unsafe { read_c_string(request) }) else {
        return error_response("the request was not a valid string");
    };

    let wallet = {
        let wallets = WALLETS.lock().unwrap_or_else(|e| e.into_inner());
        wallets.get(&handle).cloned()
    };

    let Some(wallet) = wallet else {
        return error_response("that wallet handle is not open");
    };

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        RUNTIME.block_on(async move { wallet.dispatch(&request).await })
    }));

    match outcome {
        Ok(response) => into_c_string(response),
        Err(_) => error_response("the Leos core hit an unexpected error"),
    }
}

/// Closes a wallet core.
///
/// # Safety
///
/// `handle` must be a handle returned by [`leos_wallet_new`].
#[unsafe(no_mangle)]
pub extern "C" fn leos_wallet_free(handle: u64) {
    let mut wallets = WALLETS.lock().unwrap_or_else(|e| e.into_inner());
    wallets.remove(&handle);
}

/// Releases a string returned by this library.
///
/// # Safety
///
/// `text` must have been returned by this library and not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn leos_string_free(text: *mut c_char) {
    if text.is_null() {
        return;
    }
    // SAFETY: the pointer came from `CString::into_raw` and is freed once.
    unsafe {
        drop(CString::from_raw(text));
    }
}

/// Reads a null terminated string, if there is one.
unsafe fn read_c_string(pointer: *const c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .ok()
        .map(str::to_string)
}

/// Moves a string into a C string, or reports the failure.
fn into_c_string(text: String) -> *mut c_char {
    match CString::new(text) {
        Ok(text) => text.into_raw(),
        Err(_) => error_response("the response contained a null byte"),
    }
}

/// Builds an error response.
fn error_response(message: &str) -> *mut c_char {
    let body = serde_json::json!({
        "ok": false,
        "error": {
            "code": "internal_error",
            "message": message,
            "retryable": false,
        }
    });
    into_c_string(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wallet_can_be_created_and_used() {
        let url = CString::new("http://127.0.0.1:1").unwrap();
        let handle = unsafe { leos_wallet_new(url.as_ptr()) };
        assert_ne!(handle, 0);

        let request = CString::new(r#"{"op":"format_amount","amount":"1234.5"}"#).unwrap();
        let response = unsafe { leos_wallet_call(handle, request.as_ptr()) };
        assert!(!response.is_null());

        let text = unsafe { CStr::from_ptr(response) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { leos_string_free(response) };

        assert!(text.contains("\"ok\":true"), "{text}");
        assert!(text.contains("1,234.50"), "{text}");

        leos_wallet_free(handle);
    }

    #[test]
    fn an_unknown_handle_is_reported() {
        let request = CString::new(r#"{"op":"balance"}"#).unwrap();
        let response = unsafe { leos_wallet_call(999_999, request.as_ptr()) };
        let text = unsafe { CStr::from_ptr(response) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { leos_string_free(response) };
        assert!(text.contains("not open"), "{text}");
    }

    #[test]
    fn malformed_json_is_reported_rather_than_panicking() {
        let url = CString::new("http://127.0.0.1:1").unwrap();
        let handle = unsafe { leos_wallet_new(url.as_ptr()) };

        let request = CString::new("not json at all").unwrap();
        let response = unsafe { leos_wallet_call(handle, request.as_ptr()) };
        let text = unsafe { CStr::from_ptr(response) }
            .to_str()
            .unwrap()
            .to_string();
        unsafe { leos_string_free(response) };

        assert!(text.contains("\"ok\":false"), "{text}");
        leos_wallet_free(handle);
    }
}
