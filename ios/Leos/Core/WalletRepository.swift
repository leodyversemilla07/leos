import Foundation

/// The wallet's data layer.
///
/// Everything that touches the network or the device secrets lives here.
final class WalletRepository {

    private let api: LeosAPI
    private let store: SecureStore

    private enum Keys {
        static let session = "session"
        static let phone = "phone"
        static let pinSecret = "pin_secret"
        static let deviceSecret = "device_secret"
        static let salt = "client_salt"
    }

    init(api: LeosAPI, store: SecureStore = SecureStore()) {
        self.api = api
        self.store = store
    }

    // MARK: - The wallet on this device

    /// Whether this device already holds a wallet.
    func hasWallet() -> Bool {
        store.get(Keys.phone) != nil
    }

    /// The registered mobile number, if there is one.
    func registeredPhone() -> String? {
        store.get(Keys.phone)
    }

    /// The PIN secret kept on this device.
    func storedPinSecret() -> String? {
        store.get(Keys.pinSecret)
    }

    // MARK: - Signing in

    /// Starts a registration.
    func registerStart(phone: String) -> APIResult<Challenge> {
        switch api.call("register_start", ["phone": phone]) {
        case .success(let data):
            let challenge = Challenge(
                challengeId: data["challenge_id"] as? String ?? "",
                expiresAt: data["expires_at"] as? String ?? "",
                devCode: data["dev_code"] as? String
            )
            return .success(challenge)
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// Finishes a registration with the passcode and the new PIN.
    func registerVerify(challengeId: String, code: String, pin: String, phone: String) -> APIResult<Void> {
        switch pinSecret(pin) {
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        case .success(let secret):
            let result = api.call(
                "register_verify",
                [
                    "challenge_id": challengeId,
                    "code": code,
                    "pin_secret": secret,
                ]
            )
            return persist(result, phone: phone)
        }
    }

    /// Signs in with a mobile number and a PIN.
    func login(phone: String, pin: String) -> APIResult<Void> {
        switch pinSecret(pin) {
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        case .success(let secret):
            let result = api.call("login", ["phone": phone, "pin_secret": secret])
            if case .success = result {
                store.put(Keys.pinSecret, secret)
            }
            return persist(result, phone: phone)
        }
    }

    /// Signs in with a secret that is already on the device.
    func loginWithSecret(phone: String, secret: String) -> APIResult<Void> {
        let result = api.call("login", ["phone": phone, "pin_secret": secret])
        return persist(result, phone: nil)
    }

    /// Restores a stored session into the Rust core.
    func restoreSession() -> Bool {
        guard let stored = store.get(Keys.session),
              let data = stored.data(using: .utf8),
              let session = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return false
        }
        if case .success = api.call("set_session", ["session": session]) {
            return true
        }
        return false
    }

    // MARK: - The wallet

    /// The balance of the wallet.
    func balance() -> APIResult<WalletBalance> {
        switch api.call("balance") {
        case .success(let data):
            return .success(
                WalletBalance(
                    amount: data["balance"] as? String ?? "0.00",
                    currency: data["currency"] as? String ?? "LCO"
                )
            )
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// The signed in user.
    func me() -> APIResult<UserProfile> {
        switch api.call("me") {
        case .success(let data):
            guard let user = data["user"] as? [String: Any] else {
                return .failure(code: "decode_error", message: "The profile could not be read.", retryable: false)
            }
            return .success(
                UserProfile(
                    id: user["id"] as? String ?? "",
                    phone: user["phone"] as? String ?? "",
                    displayName: user["display_name"] as? String,
                    status: user["status"] as? String ?? ""
                )
            )
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// The transaction history.
    func history(limit: Int = 50) -> APIResult<[Transaction]> {
        switch api.call("history", ["limit": limit]) {
        case .success(let data):
            let list = data["transactions"] as? [[String: Any]] ?? []
            let transactions = list.compactMap { entry -> Transaction? in
                guard let id = entry["id"] as? String else { return nil }
                return Transaction(
                    id: id,
                    transactionType: entry["transaction_type"] as? String ?? "",
                    status: entry["status"] as? String ?? "",
                    amount: entry["amount"] as? String ?? "0.00",
                    fee: entry["fee"] as? String ?? "0.00",
                    reference: entry["reference"] as? String ?? "",
                    direction: entry["direction"] as? String ?? "out",
                    counterpartyPhone: entry["counterparty_phone"] as? String,
                    counterpartyName: entry["counterparty_name"] as? String,
                    note: entry["note"] as? String,
                    createdAt: entry["created_at"] as? String ?? ""
                )
            }
            return .success(transactions)
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// Sends money to another wallet.
    func transfer(to phone: String, amount: String, note: String?, pin: String) -> APIResult<Transaction> {
        switch pinSecret(pin) {
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        case .success(let secret):
            return transaction(
                api.call(
                    "transfer",
                    [
                        "to_phone": phone,
                        "amount": amount,
                        "note": note,
                        "pin_secret": secret,
                    ]
                )
            )
        }
    }

    /// Redeems a cash in voucher.
    func redeemVoucher(code: String) -> APIResult<Transaction> {
        switch api.call("redeem", ["voucher_code": code]) {
        case .success(let data):
            guard let entry = data["transaction"] as? [String: Any],
                  let id = entry["id"] as? String else {
                return .failure(code: "decode_error", message: "The voucher could not be read.", retryable: false)
            }
            return .success(
                Transaction(
                    id: id,
                    transactionType: entry["transaction_type"] as? String ?? "cash_in",
                    status: entry["status"] as? String ?? "completed",
                    amount: entry["amount"] as? String ?? "0.00",
                    fee: entry["fee"] as? String ?? "0.00",
                    reference: entry["reference"] as? String ?? "",
                    direction: entry["direction"] as? String ?? "in",
                    counterpartyPhone: nil,
                    counterpartyName: entry["counterparty_name"] as? String,
                    note: entry["note"] as? String,
                    createdAt: entry["created_at"] as? String ?? ""
                )
            )
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// Withdraws cash through an agent.
    func cashOut(amount: String, pin: String) -> APIResult<CashOut> {
        switch pinSecret(pin) {
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        case .success(let secret):
            switch api.call("cash_out", ["amount": amount, "pin_secret": secret]) {
            case .success(let data):
                return .success(
                    CashOut(
                        id: data["id"] as? String ?? "",
                        reference: data["reference"] as? String ?? "",
                        amount: data["amount"] as? String ?? "0.00",
                        fee: data["fee"] as? String ?? "0.00",
                        instructions: data["instructions"] as? String ?? ""
                    )
                )
            case .failure(let code, let message, let retryable):
                return .failure(code: code, message: message, retryable: retryable)
            }
        }
    }

    /// Starts a top up from a bank transfer.
    func createTopup(amount: String) -> APIResult<Topup> {
        switch api.call("create_topup", ["amount": amount]) {
        case .success(let data):
            return .success(
                Topup(
                    id: data["id"] as? String ?? "",
                    reference: data["reference"] as? String ?? "",
                    amount: data["amount"] as? String ?? "0.00",
                    status: data["status"] as? String ?? "pending",
                    bankCode: data["bank_code"] as? String ?? "",
                    bankAccount: data["bank_account"] as? String ?? "",
                    expiresAt: data["expires_at"] as? String ?? ""
                )
            )
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// Signs out.
    func logout() {
        _ = api.call("logout")
        store.remove(Keys.session)
    }

    /// Forgets everything on this device.
    func forget() {
        store.remove(Keys.session)
        store.remove(Keys.phone)
    }

    // MARK: - Helpers

    /// Stretches the PIN into the secret the server expects.
    private func pinSecret(_ pin: String) -> APIResult<String> {
        let secret = store.get(Keys.deviceSecret) ?? {
            let generated = Self.randomHex(count: 32)
            store.put(Keys.deviceSecret, generated)
            return generated
        }()

        let salt = store.get(Keys.salt) ?? {
            let generated = Self.randomHex(count: 16)
            store.put(Keys.salt, generated)
            return generated
        }()

        switch api.call("derive_secret", ["pin": pin, "device_secret": secret, "salt": salt]) {
        case .success(let data):
            return .success(data["secret"] as? String ?? "")
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// Saves the session so the app can come back signed in.
    private func persist(_ result: APIResult<[String: Any]>, phone: String?) -> APIResult<Void> {
        switch result {
        case .success(let data):
            if let encoded = try? JSONSerialization.data(withJSONObject: data),
               let text = String(data: encoded, encoding: .utf8) {
                store.put(Keys.session, text)
            }
            if let phone {
                store.put(Keys.phone, phone)
            }
            return .success(())
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// Turns an operation result into a transaction.
    private func transaction(_ result: APIResult<[String: Any]>) -> APIResult<Transaction> {
        switch result {
        case .success(let data):
            guard let id = data["id"] as? String else {
                return .failure(code: "decode_error", message: "The transaction could not be read.", retryable: false)
            }
            return .success(
                Transaction(
                    id: id,
                    transactionType: data["transaction_type"] as? String ?? "",
                    status: data["status"] as? String ?? "",
                    amount: data["amount"] as? String ?? "0.00",
                    fee: data["fee"] as? String ?? "0.00",
                    reference: data["reference"] as? String ?? "",
                    direction: data["direction"] as? String ?? "out",
                    counterpartyPhone: data["counterparty_phone"] as? String,
                    counterpartyName: data["counterparty_name"] as? String,
                    note: data["note"] as? String,
                    createdAt: data["created_at"] as? String ?? ""
                )
            )
        case .failure(let code, let message, let retryable):
            return .failure(code: code, message: message, retryable: retryable)
        }
    }

    /// A random hexadecimal string.
    private static func randomHex(count: Int) -> String {
        var bytes = [UInt8](repeating: 0, count: count)
        _ = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
        return bytes.map { String(format: "%02x", $0) }.joined()
    }
}
