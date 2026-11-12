import Foundation
import SwiftUI

/// Where the app is in its journey.
enum WalletPhase {
    /// The app is starting up.
    case loading
    /// There is no wallet on this device yet.
    case onboarding
    /// There is a wallet, but the user is signed out.
    case signIn
    /// There is a wallet and a session, but the wallet is locked.
    case locked
    /// The wallet is open.
    case ready
}

/// The state the whole app renders from.
@MainActor
final class WalletViewModel: ObservableObject {

    /// The address of the Leos API.
    private let apiBaseUrl: String

    /// The wallet's data layer.
    private let repository: WalletRepository

    /// Where the app is in its journey.
    @Published var phase: WalletPhase = .loading

    /// Whether something is in flight.
    @Published var busy = false

    /// The balance.
    @Published var balance: Decimal = 0

    /// The signed in user.
    @Published var user: UserProfile?

    /// The transaction history.
    @Published var transactions: [Transaction] = []

    /// The last error, if there was one.
    @Published var error: String?

    /// The last confirmation, if there was one.
    @Published var notice: String?

    /// Creates the view model.
    init(apiBaseUrl: String = AppConfig.apiBaseUrl) {
        self.apiBaseUrl = apiBaseUrl
        self.repository = WalletRepository(api: LeosAPI(baseUrl: apiBaseUrl))
    }

    /// The mobile number this device is registered with.
    func registeredPhone() -> String? {
        repository.registeredPhone()
    }

    /// Works out where the app should start.
    func bootstrap() {
        if !repository.hasWallet() {
            phase = .onboarding
            return
        }
        phase = repository.restoreSession() ? .locked : .signIn
    }

    // MARK: - Signing in

    /// Starts a registration.
    func startRegistration(phone: String, completion: @escaping (Challenge?) -> Void) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.registerStart(phone: phone)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success(let challenge):
                    completion(challenge)
                case .failure(_, let message, _):
                    self.error = message
                    completion(nil)
                }
            }
        }
    }

    /// Finishes a registration.
    func register(phone: String, challengeId: String, code: String, pin: String) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.registerVerify(
                challengeId: challengeId,
                code: code,
                pin: pin,
                phone: phone
            )
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success:
                    self.phase = .ready
                    self.refresh()
                case .failure(_, let message, _):
                    self.error = message
                }
            }
        }
    }

    /// Signs in.
    func signIn(phone: String, pin: String) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.login(phone: phone, pin: pin)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success:
                    self.phase = .ready
                    self.refresh()
                case .failure(_, let message, _):
                    self.error = message
                }
            }
        }
    }

    /// Unlocks the wallet.
    func unlock(pin: String) {
        guard let phone = repository.registeredPhone() else { return }
        signIn(phone: phone, pin: pin)
    }

    /// Unlocks the wallet with the secret already on the device.
    func unlockWithSecret() {
        guard let phone = repository.registeredPhone(),
              let secret = repository.storedPinSecret() else {
            error = "This device cannot unlock the wallet without your PIN yet."
            return
        }
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.loginWithSecret(phone: phone, secret: secret)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success:
                    self.phase = .ready
                    self.refresh()
                case .failure(_, let message, _):
                    self.error = message
                }
            }
        }
    }

    // MARK: - The wallet

    /// Reloads the balance and the history.
    func refresh() {
        DispatchQueue.global(qos: .userInitiated).async {
            if case .success(let balance) = self.repository.balance() {
                DispatchQueue.main.async { self.balance = balance.value }
            }
            if case .success(let user) = self.repository.me() {
                DispatchQueue.main.async { self.user = user }
            }
            if case .success(let history) = self.repository.history() {
                DispatchQueue.main.async { self.transactions = history }
            }
        }
    }

    /// Sends money to another wallet.
    func transfer(to phone: String, amount: String, note: String?, pin: String, onDone: @escaping () -> Void) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.transfer(to: phone, amount: amount, note: note, pin: pin)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success:
                    self.notice = "Sent \(amount) to \(phone)"
                    self.refresh()
                    onDone()
                case .failure(_, let message, _):
                    self.error = message
                }
            }
        }
    }

    /// Redeems a cash in voucher.
    func redeemVoucher(code: String) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.redeemVoucher(code: code)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success:
                    self.notice = "Cash in credited"
                    self.refresh()
                case .failure(_, let message, _):
                    self.error = message
                }
            }
        }
    }

    /// Withdraws cash.
    func cashOut(amount: String, pin: String, onDone: @escaping (CashOut?) -> Void) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.cashOut(amount: amount, pin: pin)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success(let payout):
                    self.refresh()
                    onDone(payout)
                case .failure(_, let message, _):
                    self.error = message
                    onDone(nil)
                }
            }
        }
    }

    /// Starts a top up.
    func createTopup(amount: String, onDone: @escaping (Topup?) -> Void) {
        busy = true
        DispatchQueue.global(qos: .userInitiated).async {
            let result = self.repository.createTopup(amount: amount)
            DispatchQueue.main.async {
                self.busy = false
                switch result {
                case .success(let topup):
                    onDone(topup)
                case .failure(_, let message, _):
                    self.error = message
                    onDone(nil)
                }
            }
        }
    }

    /// Signs out.
    func signOut() {
        repository.logout()
        repository.forget()
        phase = .onboarding
        user = nil
    }

    /// Clears the message that is showing.
    func dismissMessage() {
        error = nil
        notice = nil
    }
}
