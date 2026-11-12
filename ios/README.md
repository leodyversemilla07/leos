# The Leos iOS app

SwiftUI over the same Rust core the Android app uses.

> Leos is a simulation. It holds Leos Credits, a currency that exists only
> inside this project, and it is not a registered financial service. The app
> says so on the onboarding screen, on the wallet screen and in the account
> screen.

**This app has not been compiled.** It was written on Linux, where Xcode
does not run. The Swift sources are complete and follow the same shapes as
the Android app, but nobody has built them. Treat the first build as the
first build.

## Building it

### 1. The Rust core

```bash
ios/scripts/build-rust.sh device      # for a device
ios/scripts/build-rust.sh simulator  # for the simulator
```

The script builds `libleos_ffi.a` for `aarch64-apple-ios` and, for the
simulator, for `aarch64-apple-ios-sim` and `x86_64-apple-ios`.

The targets have to be installed first:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

### 2. The app target

Create an app target in Xcode and add the sources:

| What | Where |
|---|---|
| Swift sources | `ios/Leos/` |
| The C header | `ios/Leos/Bridge/leos_ffi.h` |
| The module map | `ios/Leos/Bridge/leos_ffi.modulemap` |

Then:

1. **Build Phases > Link Binary With Libraries** — add
   `target/aarch64-apple-ios/release/libleos_ffi.a` for a device build and
   the simulator build for a simulator build.
2. **Build Settings > Swift - Objective-C Interoperability > Objective-C
   Bridging Header** — point it at `ios/Leos/Bridge/leos_ffi.h`.
3. **Build Settings > Other Swift Flags** — add
   `-Xcc -fmodule-map-file=ios/Leos/Bridge/leos_ffi.modulemap` if the
   module map is not picked up automatically.
4. Set the deployment target to iOS 16 or later.

### 3. The API address

The app reads the API address from the `LEOS_API_BASE_URL` environment
variable, and falls back to `http://127.0.0.1:8080` in debug builds. Add
that variable to the scheme, or change `AppConfig.apiBaseUrl`.

For the simulator, `127.0.0.1` is the host machine, so the server running
on the same Mac works. On a device, use the address of that machine on the
network.

ATS blocks plain HTTP, so for development add an exception in
`Info.plist`:

```xml
<key>NSAppTransportSecurity</key>
<dict>
    <key>NSExceptionDomains</key>
    <dict>
        <key>localhost</key>
        <dict>
            <key>NSExceptionAllowsInsecureHTTPLoads</key>
            <true/>
        </dict>
    </dict>
</dict>
```

The app also needs the camera permission for the QR scanner:

```xml
<key>NSCameraUsageDescription</key>
<string>Leos uses the camera to scan merchant QR codes.</string>
```

## The shape of the app

```
ios/Leos
  LeosApp.swift            the entry point and the colours
  Bridge/
    leos_ffi.h             the C header
    leos_ffi.modulemap     the module map
  Core/
    RustCore.swift         the C ABI wrapper
    LeosAPI.swift          the JSON operation wrapper
    SecureStore.swift      the keychain storage
    WalletRepository.swift the data layer
  Models/
    Models.swift           the shapes the API speaks in
  ViewModels/
    WalletViewModel.swift  the state the interface renders from
  Views/
    RootView.swift         the phase routing and the shared pieces
    OnboardingView.swift   making a wallet, signing in, the lock screen
    HomeView.swift         the wallet and the account
    MoneyViews.swift       transfer, cash, top up and the history
```

## The boundary

Swift calls the C ABI directly:

```swift
let handle = RustCore.create(baseUrl: url)
let response = RustCore.call(handle: handle, request: json)
```

The response is JSON, and the Swift side reads it with `JSONSerialization`
into plain dictionaries. That keeps the whole bridge to a handful of
functions and means no generated bindings and no version skew between the
app and the core.

## What is missing

* the QR scanner is not written for iOS yet; the Android app has the
  CameraX and ZXing implementation to follow
* Face ID is wired to the same "use the stored secret" path as the Android
  biometric unlock, but the prompt itself is not written
* no instrumented tests, and no compile check
