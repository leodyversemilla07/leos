import SwiftUI

/// The root of the app.
///
/// The phase decides which part of the app is on screen.
struct RootView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    var body: some View {
        Group {
            switch viewModel.phase {
            case .loading:
                ProgressView()
                    .onAppear { viewModel.bootstrap() }
            case .onboarding:
                OnboardingView()
            case .signIn:
                SignInView()
            case .locked:
                LockView()
            case .ready:
                HomeView()
            }
        }
        .background(Brand.background)
    }
}

/// A banner showing an error or a confirmation.
struct MessageBanner: View {

    let message: String
    let isError: Bool

    var body: some View {
        HStack {
            Text(message)
                .font(.subheadline)
                .foregroundStyle(isError ? Color.red : Brand.green)
            Spacer()
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill((isError ? Color.red : Brand.green).opacity(0.12))
        )
    }
}

/// The primary button of a screen.
struct PrimaryButton: View {

    let title: String
    var busy = false
    var enabled = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            ZStack {
                Text(title)
                    .font(.headline)
                    .opacity(busy ? 0 : 1)
                if busy {
                    ProgressView()
                        .tint(.white)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 16)
            .background(
                RoundedRectangle(cornerRadius: 16)
                    .fill(enabled ? Brand.green : Color.gray.opacity(0.4))
            )
            .foregroundStyle(.white)
        }
        .disabled(!enabled || busy)
    }
}

/// The currency of the wallet.
///
/// Leos holds Leos Credits, a currency that exists only inside this
/// project, so the amounts on screen can never be mistaken for real money.
/// Keep this in step with `crates/leos-core/src/currency.rs`.
enum Currency {
    static let symbol = "LC"
    static let name = "Leos Credits"
    static let code = "LCO"
}

/// The notice that says what this is.
///
/// Keep this in step with `crates/leos-core/src/currency.rs`.
let simulationNotice = "Leos is a simulation. It holds no real money, makes no real payments, "
    + "and is not a registered financial service."

/// The short notice for the interface.
let simulationBadge = "Simulation — no real money"

/// Formats an amount for display, e.g. `LC 1,234.56`.
func formatPeso(_ amount: String) -> String {
    guard let value = Decimal(string: amount) else { return "\(Currency.symbol) 0.00" }
    let formatter = NumberFormatter()
    formatter.numberStyle = .decimal
    formatter.minimumFractionDigits = 2
    formatter.maximumFractionDigits = 2
    return "\(Currency.symbol) " + (formatter.string(from: value as NSDecimalNumber) ?? "0.00")
}
