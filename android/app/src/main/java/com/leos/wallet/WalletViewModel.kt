package com.leos.wallet

import android.app.Application
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch
import java.math.BigDecimal

/** Where the app is in its journey. */
enum class WalletPhase {
    /** The app is starting up. */
    LOADING,

    /** There is no wallet on this device yet. */
    ONBOARDING,

    /** There is a wallet, but the user is signed out. */
    SIGN_IN,

    /** There is a wallet and a session, but the wallet is locked. */
    LOCKED,

    /** The wallet is open. */
    READY,
}

/** The state the whole app renders from. */
data class WalletUiState(
    val phase: WalletPhase = WalletPhase.LOADING,
    val busy: Boolean = false,
    val balance: BigDecimal = BigDecimal.ZERO,
    val currency: String = "LCO",
    val user: UserProfile? = null,
    val transactions: List<Transaction> = emptyList(),
    val error: String? = null,
    val notice: String? = null,
)

/**
 * The single view model behind the wallet.
 *
 * It owns the phase the app is in, the balance, and the transaction
 * history, and it is the only place that talks to the repository.
 */
class WalletViewModel(application: Application) : AndroidViewModel(application) {

    private val api = LeosApi(BuildConfig.API_BASE_URL)
    private val store = SecureStore(application)
    private val repository = WalletRepository(api, store)

    /** The state the UI renders. */
    var state by mutableStateOf(WalletUiState())
        private set

    init {
        viewModelScope.launch { bootstrap() }
    }

    /** Works out where the app should start. */
    private suspend fun bootstrap() {
        if (!repository.hasWallet()) {
            state = state.copy(phase = WalletPhase.ONBOARDING)
            return
        }

        // A stored session means the wallet is signed in, but it still has
        // to be unlocked with the PIN before anything can be seen.
        val restored = repository.restoreSession()
        state = state.copy(phase = if (restored) WalletPhase.LOCKED else WalletPhase.SIGN_IN)
    }

    /** Registers a new wallet. */
    fun register(phone: String, code: String, pin: String, onDone: () -> Unit) {
        run(busy = true) {
            val result = repository.registerVerify(challengeId = pendingChallengeId, code = code, pin = pin)
            when (result) {
                is ApiResult.Ok -> {
                    state = state.copy(phase = WalletPhase.READY)
                    refresh()
                    onDone()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** The challenge identifier of the registration in progress. */
    var pendingChallengeId: String = ""
        private set

    /** Starts a registration and returns the challenge. */
    fun startRegistration(phone: String, onCode: (Challenge) -> Unit) {
        run(busy = true) {
            when (val result = repository.registerStart(phone)) {
                is ApiResult.Ok -> {
                    pendingChallengeId = result.data.id
                    onCode(result.data)
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Signs in. */
    fun signIn(phone: String, pin: String) {
        run(busy = true) {
            val result = repository.login(phone, pin)
            when (result) {
                is ApiResult.Ok -> {
                    state = state.copy(phase = WalletPhase.READY)
                    refresh()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Unlocks the wallet with the PIN. */
    fun unlock(pin: String) {
        val phone = repository.registeredPhone() ?: return
        run(busy = true) {
            val result = repository.login(phone, pin)
            when (result) {
                is ApiResult.Ok -> {
                    state = state.copy(phase = WalletPhase.READY)
                    refresh()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** The mobile number this device is registered with. */
    fun registeredPhone(): String? = repository.registeredPhone()

    /** The PIN secret kept on this device. */
    fun storedPinSecret(): String? = repository.storedPinSecret()

    /** Unlocks the wallet with a secret that is already on the device. */
    fun unlockWithSecret(phone: String, secret: String) {
        run(busy = true) {
            val result = repository.loginWithSecret(phone, secret)
            when (result) {
                is ApiResult.Ok -> {
                    state = state.copy(phase = WalletPhase.READY)
                    refresh()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Reloads the balance and the history. */
    fun refresh() {
        viewModelScope.launch {
            when (val balance = repository.balance()) {
                is ApiResult.Ok -> state = state.copy(
                    balance = balance.data.amount,
                    currency = balance.data.currency,
                )
                is ApiResult.Err -> fail(balance.message)
            }
            when (val me = repository.me()) {
                is ApiResult.Ok -> state = state.copy(user = me.data)
                is ApiResult.Err -> fail(me.message)
            }
            when (val history = repository.history()) {
                is ApiResult.Ok -> state = state.copy(transactions = history.data)
                is ApiResult.Err -> fail(history.message)
            }
        }
    }

    /** Sends money to another wallet. */
    fun transfer(toPhone: String, amount: BigDecimal, note: String?, pin: String, onDone: () -> Unit) {
        run(busy = true) {
            when (val result = repository.transfer(toPhone, amount, note, pin)) {
                is ApiResult.Ok -> {
                    notice("Sent ${result.data.amount} to $toPhone")
                    refresh()
                    onDone()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Pays a merchant. */
    fun payMerchant(qrPayload: String, amount: BigDecimal?, pin: String, onDone: () -> Unit) {
        run(busy = true) {
            when (val result = repository.payMerchant(qrPayload, amount, pin)) {
                is ApiResult.Ok -> {
                    notice("Paid ${result.data.amount} to ${result.data.counterpartyName ?: "the merchant"}")
                    refresh()
                    onDone()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Redeems a cash in voucher. */
    fun redeemVoucher(code: String, onDone: () -> Unit) {
        run(busy = true) {
            when (val result = repository.redeemVoucher(code)) {
                is ApiResult.Ok -> {
                    notice("Cash in of ${result.data.amount} credited")
                    refresh()
                    onDone()
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Withdraws cash. */
    fun cashOut(amount: BigDecimal, pin: String, onDone: (CashOut) -> Unit) {
        run(busy = true) {
            when (val result = repository.cashOut(amount, pin)) {
                is ApiResult.Ok -> {
                    refresh()
                    onDone(result.data)
                }
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** Starts a top up. */
    fun createTopup(amount: BigDecimal, onDone: (Topup) -> Unit) {
        run(busy = true) {
            when (val result = repository.createTopup(amount)) {
                is ApiResult.Ok -> onDone(result.data)
                is ApiResult.Err -> fail(result.message)
            }
        }
    }

    /** The top ups of this wallet. */
    suspend fun topups(): List<Topup> =
        when (val result = repository.listTopups()) {
            is ApiResult.Ok -> result.data
            is ApiResult.Err -> emptyList()
        }

    /** The merchants that accept Leos payments. */
    suspend fun merchants(): List<Merchant> =
        when (val result = repository.merchants()) {
            is ApiResult.Ok -> result.data
            is ApiResult.Err -> emptyList()
        }

    /** Signs out. */
    fun signOut() {
        viewModelScope.launch {
            repository.logout()
            repository.forget()
            state = state.copy(phase = WalletPhase.ONBOARDING, user = null)
        }
    }

    /** Checks a PIN against the wallet's rules. */
    suspend fun checkPin(pin: String): Pair<Boolean, String?> = repository.validatePin(pin)

    /** Normalises a mobile number. */
    suspend fun normalizePhone(phone: String): Pair<String, String?>? = repository.normalizePhone(phone)

    /** Reads a merchant QR code on the device. */
    suspend fun parseQr(payload: String): WalletRepository.ParsedQr? = repository.parseQr(payload)

    /** Clears the error or notice that is showing. */
    fun dismissMessage() {
        state = state.copy(error = null, notice = null)
    }

    /** Reports an error from the UI layer. */
    fun fail(message: String) {
        state = state.copy(error = message, busy = false)
    }

    /** Reports a confirmation from the UI layer. */
    fun notice(message: String) {
        state = state.copy(notice = message, busy = false)
    }

    private fun run(busy: Boolean = false, block: suspend () -> Unit) {
        state = state.copy(busy = busy, error = null)
        viewModelScope.launch {
            try {
                block()
            } finally {
                state = state.copy(busy = false)
            }
        }
    }

    override fun onCleared() {
        api.close()
        super.onCleared()
    }
}
