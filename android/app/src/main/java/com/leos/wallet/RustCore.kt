package com.leos.wallet

/**
 * The bridge to the Leos Rust core.
 *
 * The Rust core holds everything that has to be right: the API client, the
 * PIN key derivation and the QR reader. The native side never handles a PIN
 * or a token itself, it only passes values across this boundary.
 */
object RustCore {

    init {
        System.loadLibrary("leos_ffi")
    }

    /** Creates a core pointed at the given API and returns its handle. */
    external fun nativeCreate(baseUrl: String): Long

    /** Runs one operation and returns the JSON response. */
    external fun nativeCall(handle: Long, request: String): String

    /** Releases a core. */
    external fun nativeFree(handle: Long)
}
