import Foundation

/// The bridge to the Leos Rust core.
///
/// Everything that has to be right — the API client, the PIN derivation,
/// the QR reader — lives in the Rust core. The Swift side passes values
/// across the boundary and renders what comes back.
enum RustCore {

    /// Creates a core pointed at the given API and returns its handle.
    static func create(baseUrl: String) -> UInt64 {
        baseUrl.withCString { leos_wallet_new($0) }
    }

    /// Runs one operation and returns the JSON response.
    static func call(handle: UInt64, request: String) -> String {
        guard let pointer = request.withCString({ leos_wallet_call(handle, $0) }) else {
            return errorEnvelope(code: "internal_error", message: "The Leos core did not answer.")
        }
        defer { leos_string_free(pointer) }
        return String(cString: pointer)
    }

    /// Releases a core.
    static func free(handle: UInt64) {
        leos_wallet_free(handle)
    }

    private static func errorEnvelope(code: String, message: String) -> String {
        "{\"ok\":false,\"error\":{\"code\":\"\(code)\",\"message\":\"\(message)\",\"retryable\":false}}"
    }
}
