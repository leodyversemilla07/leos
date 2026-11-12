import SwiftUI

/// Where the app finds the Leos API.
enum AppConfig {
    /// The base URL of the API.
    ///
    /// In development this is the machine running the server. On a device
    /// it has to be an address the device can reach.
    static let apiBaseUrl: String = {
        if let value = ProcessInfo.processInfo.environment["LEOS_API_BASE_URL"] {
            return value
        }
        #if DEBUG
        return "http://127.0.0.1:8080"
        #else
        return "https://api.leos.ph"
        #endif
    }()
}

/// The Leos wallet.
@main
struct LeosApp: App {

    /// The state behind the interface.
    @StateObject private var viewModel = WalletViewModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(viewModel)
                .tint(Brand.green)
        }
    }
}

/// The colours of the wallet.
enum Brand {
    /// The brand green.
    static let green = Color(red: 11 / 255, green: 110 / 255, blue: 79 / 255)

    /// The background of the app.
    static let background = Color(uiColor: .systemGroupedBackground)
}
