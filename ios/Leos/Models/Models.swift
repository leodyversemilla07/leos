import Foundation

/// A user as the API reports it.
struct UserProfile: Codable, Identifiable {
    let id: String
    let phone: String
    let displayName: String?
    let status: String
}

/// The wallet balance.
struct WalletBalance: Codable {
    let amount: String
    let currency: String

    /// The balance as a number, for arithmetic.
    var value: Decimal {
        Decimal(string: amount) ?? 0
    }
}

/// A transaction in the wallet history.
struct Transaction: Codable, Identifiable {
    let id: String
    let transactionType: String
    let status: String
    let amount: String
    let fee: String
    let reference: String
    let direction: String
    let counterpartyPhone: String?
    let counterpartyName: String?
    let note: String?
    let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case transactionType = "transaction_type"
        case status
        case amount
        case fee
        case reference
        case direction
        case counterpartyPhone = "counterparty_phone"
        case counterpartyName = "counterparty_name"
        case note
        case createdAt = "created_at"
    }

    /// Whether the money came in.
    var incoming: Bool { direction == "in" }
}

/// A cash out waiting to be collected.
struct CashOut: Codable, Identifiable {
    let id: String
    let reference: String
    let amount: String
    let fee: String
    let instructions: String
}

/// A top up from a bank transfer.
struct Topup: Codable, Identifiable {
    let id: String
    let reference: String
    let amount: String
    let status: String
    let bankCode: String
    let bankAccount: String
    let expiresAt: String

    enum CodingKeys: String, CodingKey {
        case id, reference, amount, status
        case bankCode = "bank_code"
        case bankAccount = "bank_account"
        case expiresAt = "expires_at"
    }
}

/// A one time passcode challenge.
struct Challenge: Codable {
    let challengeId: String
    let expiresAt: String
    let devCode: String?

    enum CodingKeys: String, CodingKey {
        case challengeId = "challenge_id"
        case expiresAt = "expires_at"
        case devCode = "dev_code"
    }
}
