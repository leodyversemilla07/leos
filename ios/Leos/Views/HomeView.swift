import SwiftUI

/// The main wallet screen.
struct HomeView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    @State private var showingTransfer = false
    @State private var showingCash = false
    @State private var showingTopUp = false

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 18) {
                    if let notice = viewModel.notice {
                        MessageBanner(message: notice, isError: false)
                    }
                    if let error = viewModel.error {
                        MessageBanner(message: error, isError: true)
                    }

                    balanceCard

                    Text(simulationBadge)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)

                    HStack(spacing: 12) {
                        quickAction("Send", "paperplane.fill") { showingTransfer = true }
                        quickAction("Cash in", "arrow.down.circle") { showingCash = true }
                        quickAction("Cash out", "banknote") { showingCash = true }
                        quickAction("Top up", "plus.circle") { showingTopUp = true }
                    }

                    Text("Recent activity")
                        .font(.headline)
                        .frame(maxWidth: .infinity, alignment: .leading)

                    if viewModel.transactions.isEmpty {
                        Text("No activity yet")
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 20)
                    } else {
                        ForEach(viewModel.transactions.prefix(6)) { transaction in
                            TransactionRow(transaction: transaction)
                        }
                    }
                }
                .padding(20)
            }
            .navigationTitle(viewModel.user?.displayName ?? "Leos")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    NavigationLink(destination: ProfileView()) {
                        Image(systemName: "person.circle")
                    }
                }
            }
            .refreshable { viewModel.refresh() }
            .sheet(isPresented: $showingTransfer) {
                TransferView()
            }
            .sheet(isPresented: $showingCash) {
                CashView()
            }
            .sheet(isPresented: $showingTopUp) {
                TopUpView()
            }
        }
    }

    private var balanceCard: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Available balance")
                .font(.subheadline)
                .foregroundStyle(.white.opacity(0.8))
            Text(formatPeso(NSDecimalNumber(decimal: viewModel.balance).stringValue))
                .font(.system(size: 36, weight: .bold))
                .foregroundStyle(.white)
            Text("\(Currency.name) (\(Currency.code))")
                .font(.caption)
                .foregroundStyle(.white.opacity(0.7))
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(24)
        .background(RoundedRectangle(cornerRadius: 24).fill(Brand.green))
    }

    private func quickAction(_ title: String, _ icon: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 8) {
                Image(systemName: icon)
                    .font(.title3)
                    .foregroundStyle(Brand.green)
                Text(title)
                    .font(.caption)
                    .foregroundStyle(.primary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 14)
            .background(RoundedRectangle(cornerRadius: 16).fill(Color(.secondarySystemGroupedBackground)))
        }
    }
}

/// One row of the transaction history.
struct TransactionRow: View {

    let transaction: Transaction

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.body)
                .foregroundStyle(transaction.incoming ? Brand.green : .secondary)
                .frame(width: 36, height: 36)
                .background(Circle().fill(Color(.secondarySystemGroupedBackground)))

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.body)
                Text(transaction.note ?? transaction.reference)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Spacer()

            Text((transaction.incoming ? "+" : "−") + formatPeso(transaction.amount))
                .font(.body.weight(.semibold))
                .foregroundStyle(transaction.incoming ? Brand.green : .primary)
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))
    }

    private var title: String {
        transaction.counterpartyName ?? transaction.counterpartyPhone ?? label
    }

    private var label: String {
        switch transaction.transactionType {
        case "transfer": return "Transfer"
        case "cash_in": return "Cash in"
        case "cash_out": return "Cash out"
        case "merchant_pay": return "Merchant payment"
        case "top_up": return "Top up"
        default: return "Transaction"
        }
    }

    private var icon: String {
        switch transaction.transactionType {
        case "transfer": return "paperplane.fill"
        case "cash_in": return "arrow.down.circle.fill"
        case "cash_out": return "banknote.fill"
        case "merchant_pay": return "qrcode"
        case "top_up": return "plus.circle.fill"
        default: return "circle.fill"
        }
    }
}

/// The account details.
struct ProfileView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    var body: some View {
        List {
            Section("Your wallet") {
                LabeledContent("Name", value: viewModel.user?.displayName ?? "—")
                LabeledContent("Mobile number", value: viewModel.user?.phone ?? "—")
                LabeledContent("Status", value: viewModel.user?.status ?? "—")
            }

            Section("About") {
                MessageBanner(message: simulationNotice, isError: false)
                LabeledContent("App", value: "Leos")
                LabeledContent("Version", value: "0.1.0")
                Text("Your PIN never leaves this phone. It is stretched into a secret with Argon2id and only that secret is sent to Leos.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }

            Section {
                Button("Sign out", role: .destructive) {
                    viewModel.signOut()
                }
            }
        }
        .navigationTitle("Account")
    }
}
