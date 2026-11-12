import Foundation

/// The outcome of an operation.
enum APIResult<T> {
    /// The operation succeeded.
    case success(T)

    /// The operation failed.
    case failure(code: String, message: String, retryable: Bool)
}

/// A thin Swift face over the Rust core.
///
/// Every call is a JSON operation, so the Swift side never has to know the
/// shape of the API and the Rust side never has to know the shape of the
/// interface.
final class LeosAPI {

    /// The wallet core this talks to.
    private var handle: UInt64

    /// Creates an API pointed at the given address.
    init(baseUrl: String) {
        handle = RustCore.create(baseUrl: baseUrl)
    }

    deinit {
        RustCore.free(handle: handle)
    }

    /// Runs an operation and returns its result.
    func call(_ op: String, _ params: [String: Any?] = [:]) -> APIResult<[String: Any]> {
        var request: [String: Any] = ["op": op]
        for (key, value) in params {
            if let value {
                request[key] = value
            }
        }

        guard let data = try? JSONSerialization.data(withJSONObject: request),
              let text = String(data: data, encoding: .utf8) else {
            return .failure(code: "invalid_request", message: "The request could not be built.", retryable: false)
        }

        let raw = RustCore.call(handle: handle, request: text)

        guard
            let data = raw.data(using: .utf8),
            let response = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            return .failure(code: "decode_error", message: "The Leos core sent something unreadable.", retryable: false)
        }

        if (response["ok"] as? Bool) == true {
            let payload = response["data"] as? [String: Any] ?? [:]
            return .success(payload)
        }

        let error = response["error"] as? [String: Any] ?? [:]
        return .failure(
            code: error["code"] as? String ?? "unknown_error",
            message: error["message"] as? String ?? "Something went wrong.",
            retryable: error["retryable"] as? Bool ?? false
        )
    }
}
