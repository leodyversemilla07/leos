# The Leos Android app

Kotlin and Jetpack Compose over the Rust core in `crates/leos-ffi`.

> Leos is a simulation. It holds Leos Credits, a currency that exists only
> inside this project, and it is not a registered financial service. The app
> says so on the onboarding screen, on the wallet screen and in the account
> screen.

## Building it

The Gradle build compiles the Rust core for every ABI with the NDK and
packages the shared libraries into the APK, so there is no separate step.

```bash
cd android
./gradlew :app:assembleDebug
```

The APK lands in `app/build/outputs/apk/debug/app-debug.apk`.

### What the build needs

* the Android SDK, with `sdk.dir` in `android/local.properties`
* the NDK (the build finds it in `$ANDROID_NDK_HOME` or the newest version
  in `$ANDROID_SDK_ROOT/ndk`)
* a Rust toolchain with the Android targets:
  ```bash
  rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
  ```

The NDK's `clang` wrappers are used for both linking and the C compiler
that the crypto library needs, so the build sets `CC_<target>` and
`AR_<target>` for each architecture.

## Pointing it at the server

The address of the API is a build configuration field. Put it in
`android/local.properties`:

```properties
leos.api.baseUrl=http://10.0.2.2:8080
```

`10.0.2.2` is how the Android emulator reaches the machine it is running
on. On a physical device, use the address of that machine on the network.

The app allows cleartext HTTP only for the loopback addresses
(`10.0.2.2`, `localhost` and `127.0.0.1`); everything else has to be
HTTPS.

## Running it

```bash
# The API, on the machine
./scripts/dev-server.sh start

# The app
cd android && ./gradlew :app:installDebug
```

The seeded demo wallets are (all in Leos Credits):

| Number | Name | PIN |
|---|---|---|
| +63 917 000 0001 | Ana Reyes | 4821 |
| +63 917 000 0002 | Bello Santos | 4821 |

The passcode is shown by the app in development builds, because the server
returns it when `LEOS_DEV_MODE` is on.

## The shape of the app

```
com.leos.wallet
  RustCore.kt            the JNI entry points
  LeosApi.kt             the JSON operation wrapper
  SecureStore.kt         the keystore backed storage
  WalletRepository.kt    the data layer
  WalletViewModel.kt     the state the interface renders from
  MainActivity.kt        the single activity
  ui/
    LeosApp.kt           the phase routing and the navigation
    theme/               the colours
    components/          the buttons, fields and cards
    screens/             the screens
```

There is one view model. The wallet is small enough that splitting it
would add ceremony without adding clarity, and one state object makes the
phase the app is in (`onboarding`, `sign in`, `locked`, `ready`) easy to
follow.

### How a screen talks to the core

Nothing in the interface handles a PIN, a token or an amount itself. A
screen calls the view model, the view model calls the repository, and the
repository asks the Rust core to do the work:

```kotlin
viewModel.transfer(toPhone, amount, note, pin) { done -> … }
```

The amount arrives as a `BigDecimal`, the PIN goes to the Rust core, and
the core is what decides whether the PIN is acceptable and what secret to
send.

The currency symbol and the simulation notice are constants in
`ui/components/Common.kt`, kept in step with
`crates/leos-core/src/currency.rs`.

## The camera

The QR scanner uses CameraX for the preview and ZXing for the decoding, on
a background thread, so the interface stays smooth. Only the luminance
plane of each frame is read, which is all ZXing needs.

The decoded payload is handed to the Rust core, which parses it and checks
its checksum before the app shows the merchant. A payload that is not a
merchant QR is rejected on the device.

## Biometrics

The lock screen offers a biometric unlock when the phone has a biometric
enrolled. The prompt runs through `androidx.biometric`, and on success the
app signs in with the PIN secret that is already in the keystore, so the
user does not have to type their PIN.

## The tests

There are no instrumented tests yet. The rules that matter are tested in
the Rust core:

```bash
cargo test -p leos-core
cargo test -p leos-ffi
```
