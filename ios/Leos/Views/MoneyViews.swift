import SwiftUI

/// Sending money to another wallet.
struct TransferView: View {

    @EnvironmentObject private var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss

    @State private var phone = ""
    @State private var amount = ""
    @State private var note = ""
    @State private var pin = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Recipient's mobile number", text: $phone)
                        .keyboardType(.phonePad)
                    TextField("Amount", text: $amount)
                        .keyboardType(.decimalPad)
                    TextField("Note (optional)", text: $note)
                }

                Section {
                    SecureField("PIN", text: $pin)
                        .keyboardType(.numberPad)
                }

                if let error = viewModel.error {
                    Section {
                        MessageBanner(message: error, isError: true)
                    }
                }

                Section {
                    PrimaryButton(title: "Send", busy: viewModel.busy, enabled: !phone.isEmpty && !amount.isEmpty && pin.count >= 4) {
                        viewModel.transfer(to: phone, amount: amount, note: note.isEmpty ? nil : note, pin: pin) {
                            dismiss()
                        }
                    }
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
                }
            }
            .navigationTitle("Send money")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}

/// Cash in and cash out.
struct CashView: View {

    @EnvironmentObject private var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss

    @State private var cashingIn = true
    @State private var code = ""
    @State private var amount = ""
    @State private var pin = ""
    @State private var payout: CashOut?

    var body: some View {
        NavigationStack {
            Form {
                Picker("Direction", selection: $cashingIn) {
                    Text("Cash in").tag(true)
                    Text("Cash out").tag(false)
                }
                .pickerStyle(.segmented)

                if cashingIn {
                    Section("Voucher code") {
                        TextField("ABCD2345", text: $code)
                            .textInputAutocapitalization(.characters)
                            .autocorrectionDisabled()
                    }
                    Section {
                        PrimaryButton(title: "Credit my wallet", busy: viewModel.busy, enabled: !code.isEmpty) {
                            viewModel.redeemVoucher(code: code)
                            code = ""
                        }
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                    }
                } else if let payout {
                    Section("Collect your cash") {
                        LabeledContent("Reference", value: payout.reference)
                        LabeledContent("Amount", value: formatPeso(payout.amount))
                        LabeledContent("Fee", value: formatPeso(payout.fee))
                    }
                    Section {
                        PrimaryButton(title: "Done") { dismiss() }
                            .listRowInsets(EdgeInsets())
                            .listRowBackground(Color.clear)
                    }
                } else {
                    Section {
                        TextField("Amount", text: $amount)
                            .keyboardType(.decimalPad)
                        SecureField("PIN", text: $pin)
                            .keyboardType(.numberPad)
                    }
                    Section {
                        PrimaryButton(title: "Request cash", busy: viewModel.busy, enabled: !amount.isEmpty && pin.count >= 4) {
                            viewModel.cashOut(amount: amount, pin: pin) { result in
                                payout = result
                            }
                        }
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                    }
                }

                if let error = viewModel.error {
                    Section {
                        MessageBanner(message: error, isError: true)
                    }
                }
            }
            .navigationTitle("Cash")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }
}

/// Topping up from a bank transfer.
struct TopUpView: View {

    @EnvironmentObject private var viewModel: WalletViewModel
    @Environment(\.dismiss) private var dismiss

    @State private var amount = ""
    @State private var topup: Topup?

    var body: some View {
        NavigationStack {
            Form {
                if let topup {
                    Section("Transfer this amount") {
                        LabeledContent("Amount", value: formatPeso(topup.amount))
                        LabeledContent("Reference", value: topup.reference)
                        LabeledContent("Bank", value: topup.bankCode)
                        LabeledContent("Account", value: topup.bankAccount)
                    }
                    Section {
                        Text("Send the transfer and quote the reference exactly. The wallet is credited as soon as the bank confirms.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }
                } else {
                    Section {
                        TextField("Amount", text: $amount)
                            .keyboardType(.decimalPad)
                    }
                    Section {
                        PrimaryButton(title: "Get a reference", busy: viewModel.busy, enabled: !amount.isEmpty) {
                            viewModel.createTopup(amount: amount) { result in
                                topup = result
                            }
                        }
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                    }
                }

                if let error = viewModel.error {
                    Section {
                        MessageBanner(message: error, isError: true)
                    }
                }
            }
            .navigationTitle("Top up")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
    }
}

/// The full transaction history.
struct HistoryView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    @State private var onlyIncoming = false

    var body: some View {
        List {
            ForEach(filtered) { transaction in
                TransactionRow(transaction: transaction)
                    .listRowInsets(EdgeInsets())
                    .listRowSeparator(.hidden)
            }
        }
        .navigationTitle("History")
    }

    private var filtered: [Transaction] {
        onlyIncoming ? viewModel.transactions.filter { $0.incoming } : viewModel.transactions
    }
}
