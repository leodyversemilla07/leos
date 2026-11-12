/*
 * The Leos Rust core, as seen by Swift.
 *
 * The Swift side of the wallet never touches a PIN, a token or the API
 * directly: it passes a JSON request in and reads a JSON response out, and
 * the Rust core does the rest.
 */

#ifndef LEOS_FFI_H
#define LEOS_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Creates a wallet core pointed at an API.
 *
 * Returns a handle, or zero if the address was not usable.
 */
uint64_t leos_wallet_new(const char *base_url);

/**
 * Runs an operation and returns the JSON response.
 *
 * The returned string is owned by Rust and must be released with
 * leos_string_free.
 */
char *leos_wallet_call(uint64_t handle, const char *request);

/**
 * Closes a wallet core.
 */
void leos_wallet_free(uint64_t handle);

/**
 * Releases a string returned by this library.
 */
void leos_string_free(char *text);

#ifdef __cplusplus
}
#endif

#endif /* LEOS_FFI_H */
