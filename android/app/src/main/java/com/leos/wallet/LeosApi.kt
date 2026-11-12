package com.leos.wallet

import org.json.JSONObject

/** The outcome of an operation. */
sealed interface ApiResult<out T> {
    /** The operation succeeded. */
    data class Ok<T>(val data: T) : ApiResult<T>

    /** The operation failed. */
    data class Err(
        val code: String,
        val message: String,
        val retryable: Boolean,
    ) : ApiResult<Nothing>
}

/** Maps the value of a successful result, leaving a failure alone. */
fun <T, R> ApiResult<T>.map(transform: (T) -> R): ApiResult<R> = when (this) {
    is ApiResult.Ok -> ApiResult.Ok(transform(data))
    is ApiResult.Err -> this
}

/**
 * A thin Kotlin face over the Rust core.
 *
 * Every call is a JSON operation, so the native side never has to know the
 * shape of the API and the Rust side never has to know the shape of the UI.
 */
class LeosApi(baseUrl: String) {

    private var handle: Long = RustCore.nativeCreate(baseUrl)

    init {
        if (handle == 0L) {
            throw IllegalStateException("the Leos core could not be started")
        }
    }

    /** Runs an operation and returns its result. */
    fun call(op: String, params: Map<String, Any?> = emptyMap()): ApiResult<JSONObject> {
        val request = JSONObject().apply {
            put("op", op)
            params.forEach { (key, value) ->
                if (value != null) {
                    put(key, value)
                }
            }
        }

        val raw = RustCore.nativeCall(handle, request.toString())
        val response = JSONObject(raw)

        return if (response.optBoolean("ok")) {
            ApiResult.Ok(response.optJSONObject("data") ?: JSONObject())
        } else {
            val error = response.optJSONObject("error") ?: JSONObject()
            ApiResult.Err(
                code = error.optString("code", "unknown_error"),
                message = error.optString("message", "Something went wrong."),
                retryable = error.optBoolean("retryable", false),
            )
        }
    }

    /** Releases the core. */
    fun close() {
        if (handle != 0L) {
            RustCore.nativeFree(handle)
            handle = 0L
        }
    }
}
