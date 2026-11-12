import SwiftUI

/// Making a wallet: the mobile number, the passcode, and the PIN.
struct OnboardingView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    @State private var step = 0
    @State private var phone = ""
    @State private var code = ""
    @State private var pin = ""
    @State private var pinAgain = ""
    @State private var challengeId = ""
    @State private var devCode: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("Leos")
                    .font(.largeTitle.bold())
                    .foregroundStyle(Brand.green)

                Text(subtitle)
                    .font(.body)
                    .foregroundStyle(.secondary)

                MessageBanner(message: simulationNotice, isError: false)

                if let error = viewModel.error {
                    MessageBanner(message: error, isError: true)
                }

                switch step {
                case 0:
                    TextField("Mobile number", text: $phone)
                        .keyboardType(.phonePad)
                        .textContentType(.telephoneNumber)
                        .padding(14)
                        .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

                    Text("We will text you a one time code to confirm the number is yours.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)

                    PrimaryButton(title: "Send code", busy: viewModel.busy, enabled: !phone.isEmpty) {
                        viewModel.startRegistration(phone: phone) { challenge in
                            if let challenge {
                                challengeId = challenge.challengeId
                                devCode = challenge.devCode
                                step = 1
                            }
                        }
                    }

                case 1:
                    TextField("One time code", text: $code)
                        .keyboardType(.numberPad)
                        .textContentType(.oneTimeCode)
                        .padding(14)
                        .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

                    if let devCode {
                        Text("Development code: \(devCode)")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                    }

                    PrimaryButton(title: "Continue", busy: viewModel.busy, enabled: code.count == 6) {
                        step = 2
                    }

                default:
                    SecureField("PIN", text: $pin)
                        .keyboardType(.numberPad)
                        .padding(14)
                        .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

                    SecureField("Confirm PIN", text: $pinAgain)
                        .keyboardType(.numberPad)
                        .padding(14)
                        .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

                    PrimaryButton(title: "Create wallet", busy: viewModel.busy, enabled: pin.count >= 4 && !pinAgain.isEmpty) {
                        guard pin == pinAgain else {
                            viewModel.error = "The two PINs do not match."
                            return
                        }
                        viewModel.register(phone: phone, challengeId: challengeId, code: code, pin: pin)
                    }
                }
            }
            .padding(24)
        }
    }

    private var subtitle: String {
        switch step {
        case 0: return "A wallet for everyday payments in the Philippines."
        case 1: return "We sent a six digit code to \(phone)."
        default: return "Choose a PIN to protect your wallet."
        }
    }
}

/// Signing in to a wallet that is already on this device.
struct SignInView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    @State private var phone = ""
    @State private var pin = ""

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            Text("Leos")
                .font(.largeTitle.bold())
                .foregroundStyle(Brand.green)
            Text("Sign in to your wallet")
                .foregroundStyle(.secondary)

            if let error = viewModel.error {
                MessageBanner(message: error, isError: true)
            }

            TextField("Mobile number", text: $phone)
                .keyboardType(.phonePad)
                .padding(14)
                .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

            SecureField("PIN", text: $pin)
                .keyboardType(.numberPad)
                .padding(14)
                .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

            PrimaryButton(title: "Sign in", busy: viewModel.busy, enabled: !phone.isEmpty && pin.count >= 4) {
                viewModel.signIn(phone: phone, pin: pin)
            }

            Button("Use a different wallet") {
                viewModel.signOut()
            }
            .font(.footnote)

            Spacer()
        }
        .padding(24)
        .onAppear {
            if phone.isEmpty {
                phone = viewModel.registeredPhone() ?? ""
            }
        }
    }
}

/// The lock screen.
struct LockView: View {

    @EnvironmentObject private var viewModel: WalletViewModel

    @State private var pin = ""

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            Text("Leos")
                .font(.largeTitle.bold())
                .foregroundStyle(Brand.green)
            Text("Enter your PIN to open your wallet")
                .foregroundStyle(.secondary)

            if let error = viewModel.error {
                MessageBanner(message: error, isError: true)
            }

            SecureField("PIN", text: $pin)
                .keyboardType(.numberPad)
                .padding(14)
                .background(RoundedRectangle(cornerRadius: 14).fill(Color(.secondarySystemGroupedBackground)))

            PrimaryButton(title: "Unlock", busy: viewModel.busy, enabled: pin.count >= 4) {
                viewModel.unlock(pin: pin)
            }

            Button("Use Face ID") {
                viewModel.unlockWithSecret()
            }
            .font(.footnote)

            Spacer()
        }
        .padding(24)
    }
}
