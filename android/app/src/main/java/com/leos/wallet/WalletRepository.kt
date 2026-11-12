package com.leos.wallet

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.math.BigDecimal

/** The signed in user. */
data class UserProfile(
    val id: String,
    val phone: String,
    val displayName: String?,
    val status: String,
)

/** The wallet balance. */
data class WalletBalance(
    val amount: BigDecimal,
    val currency: String,
)

/** A transaction in the wallet history. */
data class Transaction(
    val id: String,
    val type: String,
    val status: String,
    val amount: BigDecimal,
    val fee: BigDecimal,
    val reference: String,
    val direction: String,
    val counterpartyPhone: String?,
    val counterpartyName: String?,
    val note: String?,
    val createdAt: String,
) {
    /** Whether the money came in. */
    val incoming: Boolean get() = direction == "in"
}

/** A cash out waiting to be collected. */
data class CashOut(
    val id: String,
    val reference: String,
    val amount: BigDecimal,
    val fee: BigDecimal,
    val instructions: String,
)

/** A top up from a bank transfer. */
data class Topup(
    val id: String,
    val reference: String,
    val amount: BigDecimal,
    val status: String,
    val bankCode: String,
    val bankAccount: String,
    val expiresAt: String,
)

/** A one time passcode challenge. */
data class Challenge(
    val id: String,
    val expiresAt: String,
    val devCode: String?,
)

/** A merchant that accepts Leos payments. */
data class Merchant(
    val id: String,
    val name: String,
    val city: String,
    val qrPayload: String,
)

/**
 * The wallet's data layer.
 *
 * Everything that touches the network or the device secrets lives here, and
 * every method blocks the calling thread, so they are all wrapped in
 * `Dispatchers.IO`.
 */
class WalletRepository(
    private val api: LeosApi,
    private val store: SecureStore,
) {

    /** Whether this device already holds a wallet. */
    fun hasWallet(): Boolean = store.contains(KEY_PHONE)

    /** The registered mobile number, if there is one. */
    fun registeredPhone(): String? = store.get(KEY_PHONE)

    /** Whether the wallet is currently unlocked. */
    fun isUnlocked(): Boolean = store.contains(KEY_SESSION)

    /** Restores a stored session into the Rust core. */
    suspend fun restoreSession(): Boolean = withContext(Dispatchers.IO) {
        val stored = store.get(KEY_SESSION) ?: return@withContext false
        when (val result = api.call("set_session", mapOf("session" to JSONObject(stored)))) {
            is ApiResult.Ok -> true
            is ApiResult.Err -> false
        }
    }

    /** Starts a registration and returns the challenge. */
    suspend fun registerStart(phone: String): ApiResult<Challenge> = call("register_start", mapOf("phone" to phone)) {
        Challenge(
            id = it.getString("challenge_id"),
            expiresAt = it.getString("expires_at"),
            devCode = it.optString("dev_code").takeIf { code -> code.isNotEmpty() },
        )
    }

    /** Finishes a registration with the passcode and the new PIN. */
    suspend fun registerVerify(challengeId: String, code: String, pin: String): ApiResult<Unit> =
        withContext(Dispatchers.IO) {
            val secret = pinSecret(pin)
            if (secret is ApiResult.Err) {
                return@withContext secret
            }

            val result = api.call(
                "register_verify",
                mapOf(
                    "challenge_id" to challengeId,
                    "code" to code,
                    "pin_secret" to (secret as ApiResult.Ok).data.getString("secret"),
                ),
            )
            if (result is ApiResult.Ok) {
                persistSession(result.data, phone = null)
            }
            result.map { }
        }

    /** Signs in with a mobile number and a PIN. */
    suspend fun login(phone: String, pin: String): ApiResult<Unit> = withContext(Dispatchers.IO) {
        val secret = pinSecret(pin)
        if (secret is ApiResult.Err) {
            return@withContext secret
        }

        val value = (secret as ApiResult.Ok).data.getString("secret")
        val result = api.call("login", mapOf("phone" to phone, "pin_secret" to value))
        if (result is ApiResult.Ok) {
            persistSession(result.data, phone = phone)
            store.put(KEY_PIN_SECRET, value)
        }
        result.map { }
    }

    /**
     * Signs in with a secret that was derived earlier.
     *
     * This is how the biometric unlock works: the secret is already on the
     * device, protected by the keystore, so the user does not have to type
     * their PIN.
     */
    suspend fun loginWithSecret(phone: String, secret: String): ApiResult<Unit> =
        withContext(Dispatchers.IO) {
            val result = api.call("login", mapOf("phone" to phone, "pin_secret" to secret))
            if (result is ApiResult.Ok) {
                persistSession(result.data, phone = phone)
            }
            result.map { }
        }

    /** The PIN secret kept on this device, if there is one. */
    fun storedPinSecret(): String? = store.get(KEY_PIN_SECRET)

    /** The balance of the wallet. */
    suspend fun balance(): ApiResult<WalletBalance> = call("balance") {
        WalletBalance(
            amount = it.getString("balance").toBigDecimal(),
            currency = it.getString("currency"),
        )
    }

    /** The signed in user. */
    suspend fun me(): ApiResult<UserProfile> = call("me") {
        parseUser(it.getJSONObject("user"))
    }

    /** The transaction history. */
    suspend fun history(limit: Int = 50): ApiResult<List<Transaction>> =
        call("history", mapOf("limit" to limit)) { data ->
            val list = data.getJSONArray("transactions")
            List(list.length()) { index -> parseTransaction(list.getJSONObject(index)) }
        }

    /** Sends money to another wallet. */
    suspend fun transfer(
        toPhone: String,
        amount: BigDecimal,
        note: String?,
        pin: String,
    ): ApiResult<Transaction> = withContext(Dispatchers.IO) {
        val secret = pinSecret(pin)
        if (secret is ApiResult.Err) {
            return@withContext secret
        }
        call(
            "transfer",
            mapOf(
                "to_phone" to toPhone,
                "amount" to amount.toPlainString(),
                "note" to note?.takeIf { it.isNotBlank() },
                "pin_secret" to (secret as ApiResult.Ok).data.getString("secret"),
            ),
        ) { parseTransaction(it) }
    }

    /** Pays a merchant by their QR code. */
    suspend fun payMerchant(
        qrPayload: String,
        amount: BigDecimal?,
        pin: String,
    ): ApiResult<Transaction> = withContext(Dispatchers.IO) {
        val secret = pinSecret(pin)
        if (secret is ApiResult.Err) {
            return@withContext secret
        }
        call(
            "pay_merchant",
            mapOf(
                "qr_payload" to qrPayload,
                "amount" to amount?.toPlainString(),
                "pin_secret" to (secret as ApiResult.Ok).data.getString("secret"),
            ),
        ) { parseTransaction(it) }
    }

    /** Redeems a cash in voucher. */
    suspend fun redeemVoucher(code: String): ApiResult<Transaction> =
        call("redeem", mapOf("voucher_code" to code)) {
            parseTransaction(it.getJSONObject("transaction"))
        }

    /** Withdraws cash through an agent. */
    suspend fun cashOut(amount: BigDecimal, pin: String): ApiResult<CashOut> =
        withContext(Dispatchers.IO) {
            val secret = pinSecret(pin)
            if (secret is ApiResult.Err) {
                return@withContext secret
            }
            call(
                "cash_out",
                mapOf(
                    "amount" to amount.toPlainString(),
                    "pin_secret" to (secret as ApiResult.Ok).data.getString("secret"),
                ),
            ) {
                CashOut(
                    id = it.getString("id"),
                    reference = it.getString("reference"),
                    amount = it.getString("amount").toBigDecimal(),
                    fee = it.getString("fee").toBigDecimal(),
                    instructions = it.getString("instructions"),
                )
            }
        }

    /** Starts a top up from a bank transfer. */
    suspend fun createTopup(amount: BigDecimal): ApiResult<Topup> =
        call("create_topup", mapOf("amount" to amount.toPlainString())) {
            parseTopup(it)
        }

    /** The top ups of this wallet. */
    suspend fun listTopups(): ApiResult<List<Topup>> = call("list_topups") { data ->
        val list = data.getJSONArray("topups")
        List(list.length()) { index -> parseTopup(list.getJSONObject(index)) }
    }

    /** The merchants that accept Leos payments. */
    suspend fun merchants(): ApiResult<List<Merchant>> = call("merchants") { data ->
        val list = data.getJSONArray("merchants")
        List(list.length()) { index ->
            val merchant = list.getJSONObject(index)
            Merchant(
                id = merchant.getString("id"),
                name = merchant.getString("name"),
                city = merchant.getString("city"),
                qrPayload = merchant.getString("qr_payload"),
            )
        }
    }

    /** A merchant QR that has been read on the device. */
    data class ParsedQr(
        val merchantId: String?,
        val merchantName: String?,
        val merchantCity: String?,
        val amount: String?,
    )

    /**
     * Reads a merchant QR code on the device.
     *
     * The code is checked here, before anything is sent to the server, so a
     * code that is not a Leos QR never leaves the phone.
     */
    suspend fun parseQr(payload: String): ParsedQr? = withContext(Dispatchers.IO) {
        when (val result = api.call("parse_qr", mapOf("payload" to payload))) {
            is ApiResult.Ok -> ParsedQr(
                merchantId = result.data.optString("merchant_id").takeIf { it.isNotEmpty() },
                merchantName = result.data.optString("merchant_name").takeIf { it.isNotEmpty() },
                merchantCity = result.data.optString("merchant_city").takeIf { it.isNotEmpty() },
                amount = result.data.optString("amount").takeIf { it.isNotEmpty() },
            )
            is ApiResult.Err -> null
        }
    }

    /** Checks that a PIN satisfies the wallet's rules. */
    suspend fun validatePin(pin: String): Pair<Boolean, String?> = withContext(Dispatchers.IO) {
        when (val result = api.call("validate_pin", mapOf("pin" to pin))) {
            is ApiResult.Ok -> {
                val valid = result.data.optBoolean("valid", false)
                valid to result.data.optString("reason").takeIf { it.isNotEmpty() }
            }
            is ApiResult.Err -> false to result.message
        }
    }

    /** Normalises a mobile number the way the server does. */
    suspend fun normalizePhone(phone: String): Pair<String, String?>? = withContext(Dispatchers.IO) {
        when (val result = api.call("normalize_phone", mapOf("phone" to phone))) {
            is ApiResult.Ok -> result.data.getString("e164") to
                result.data.optString("masked").takeIf { it.isNotEmpty() }
            is ApiResult.Err -> null
        }
    }

    /** Signs out and forgets the session on this device. */
    suspend fun logout() = withContext(Dispatchers.IO) {
        api.call("logout")
        store.remove(KEY_SESSION)
    }

    /** Forgets everything, including the registered number. */
    fun forget() {
        store.remove(KEY_SESSION)
        store.remove(KEY_PHONE)
    }

    /**
     * Stretches the PIN into the secret the server expects.
     *
     * This never leaves the device in the clear.
     */
    private fun pinSecret(pin: String): ApiResult<JSONObject> {
        return api.call(
            "derive_secret",
            mapOf(
                "pin" to pin,
                "device_secret" to store.deviceSecret(),
                "salt" to store.salt(),
            ),
        )
    }

    /** Saves the session so the app can come back signed in. */
    private fun persistSession(data: JSONObject, phone: String?) {
        store.put(KEY_SESSION, data.toString())
        phone?.let { store.put(KEY_PHONE, it) }
    }

    /** Runs an operation and maps the result. */
    private suspend fun <T> call(
        op: String,
        params: Map<String, Any?> = emptyMap(),
        parse: (JSONObject) -> T,
    ): ApiResult<T> = withContext(Dispatchers.IO) {
        when (val result = api.call(op, params)) {
            is ApiResult.Ok -> ApiResult.Ok(parse(result.data))
            is ApiResult.Err -> result
        }
    }

    private fun parseUser(json: JSONObject): UserProfile {
        return UserProfile(
            id = json.getString("id"),
            phone = json.getString("phone"),
            displayName = json.optString("display_name").takeIf { it.isNotEmpty() },
            status = json.getString("status"),
        )
    }

    private fun parseTransaction(json: JSONObject): Transaction {
        return Transaction(
            id = json.getString("id"),
            type = json.getString("transaction_type"),
            status = json.getString("status"),
            amount = json.getString("amount").toBigDecimal(),
            fee = json.optString("fee", "0.00").toBigDecimal(),
            reference = json.getString("reference"),
            direction = json.getString("direction"),
            counterpartyPhone = json.optString("counterparty_phone").takeIf { it.isNotEmpty() },
            counterpartyName = json.optString("counterparty_name").takeIf { it.isNotEmpty() },
            note = json.optString("note").takeIf { it.isNotEmpty() },
            createdAt = json.getString("created_at"),
        )
    }

    private fun parseTopup(json: JSONObject): Topup {
        return Topup(
            id = json.getString("id"),
            reference = json.getString("reference"),
            amount = json.getString("amount").toBigDecimal(),
            status = json.getString("status"),
            bankCode = json.getString("bank_code"),
            bankAccount = json.optString("bank_account"),
            expiresAt = json.getString("expires_at"),
        )
    }

    private companion object {
        const val KEY_SESSION = "session"
        const val KEY_PHONE = "phone"
        const val KEY_PIN_SECRET = "pin_secret"
    }
}
