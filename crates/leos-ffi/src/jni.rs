//! The JNI entry points used by the Android app.
//!
//! They are thin wrappers over the same C ABI the iOS app uses, so both
//! platforms run identical code.

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jlong, jstring};

use crate::{leos_string_free, leos_wallet_call, leos_wallet_free, leos_wallet_new};
use std::ffi::CString;

/// Creates a wallet core and returns its handle.
///
/// # Safety
///
/// Called by the JVM with valid arguments.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_leos_wallet_RustCore_nativeCreate(
    mut env: JNIEnv,
    _class: JClass,
    base_url: JString,
) -> jlong {
    let Ok(url) = env.get_string(&base_url) else {
        return 0;
    };
    let url: String = url.into();
    let Ok(url) = CString::new(url) else {
        return 0;
    };

    (unsafe { leos_wallet_new(url.as_ptr()) }) as jlong
}

/// Runs an operation and returns the JSON response.
///
/// # Safety
///
/// Called by the JVM with valid arguments.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_leos_wallet_RustCore_nativeCall(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    request: JString,
) -> jstring {
    let Ok(request) = env.get_string(&request) else {
        return std::ptr::null_mut();
    };
    let request: String = request.into();
    let Ok(request) = CString::new(request) else {
        return std::ptr::null_mut();
    };

    let response = unsafe { leos_wallet_call(handle as u64, request.as_ptr()) };
    if response.is_null() {
        return std::ptr::null_mut();
    }

    let text = unsafe { std::ffi::CStr::from_ptr(response) };
    let text = match text.to_str() {
        Ok(text) => text.to_string(),
        Err(_) => {
            unsafe { leos_string_free(response) };
            return std::ptr::null_mut();
        }
    };
    unsafe { leos_string_free(response) };

    match env.new_string(text) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Closes a wallet core.
///
/// # Safety
///
/// Called by the JVM with valid arguments.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_leos_wallet_RustCore_nativeFree(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    leos_wallet_free(handle as u64);
}
