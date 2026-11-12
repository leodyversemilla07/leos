# Leos

> ### ⚠️ This is a simulation
>
> Leos is a personal development project, not a financial service. It holds
> **Leos Credits (LC)** — a currency that exists only inside this repository —
> so no balance can ever be mistaken for real money.
>
> * It holds **no real money** and makes **no real payments**.
> * It is **not a registered financial service** and has no licence, no
>   users and no funds.
> * Nothing is connected to a bank, a payment network, an acquirer or a
>   real merchant. The agent terminal, the bank and the merchants are all
>   simulated locally.
>
> What *is* real is the engineering: the double-entry ledger, the
> idempotency, the Argon2id PIN derivation, the QR Ph codec, the limits
> and the concurrency handling. That is what the project is for.

A digital wallet for the Philippines, written in Rust with native mobile
interfaces.

Leos holds *Leos Credits*. You can send money to another person, pay a
merchant by scanning their QR code, take cash out through a partner agent,
and top up from a bank. The phone is the interface, but the *server*
decides what a balance is: the app is a client of an authoritative ledger,
not a place where money is kept.

The Philippine context is deliberate. The phone numbers, the QR Ph standard,
the cash agent network and the bank top up are all the real mechanics of a
Philippine e-wallet, because those are the mechanics worth learning. Only
the currency is invented.

```
┌──────────────────────┐        ┌──────────────────────┐
│  Android             │        │  iOS                 │
│  Kotlin + Compose    │        │  SwiftUI             │
└──────────┬───────────┘        └──────────┬───────────┘
           │                               │
           │   JSON over a C ABI and JNI  │
           └───────────────┬───────────────┘
                           │
                  ┌────────▼─────────┐
                  │  leos-core       │  money, phones, QR, ledger rules
                  │  leos-client     │  API client, PIN derivation
                  │  leos-ffi        │  the boundary
                  └────────┬─────────┘
                           │  HTTPS + JSON
                  ┌────────▼─────────┐
                  │  leos-server     │  axum + Postgres
                  │  double entry    │  the authoritative ledger
                  └──────────────────┘
```

## What is here

| Crate | What it does |
|---|---|
| `crates/leos-core` | Shared rules: amounts as integer centavos, Philippine phone numbers, the EMVCo/QR Ph codec, PIN policy, limits, fees, the currency and the ledger types. No I/O. |
| `crates/leos-client` | The on-device core: the API client, the PIN key derivation and the QR reader. This is the code the phone runs. |
| `crates/leos-ffi` | The boundary. A C ABI for iOS and JNI entry points for Android, over one implementation. |
| `crates/leos-server` | The API. Owns the ledger, the limits and the payment rails. |
| `crates/leos-cli` | The other side of the rails: a cash agent terminal and a bank, so the whole flow runs without real money moving. |
| `android/` | The Android app: Kotlin, Jetpack Compose, CameraX and ZXing. |
| `ios/` | The iOS app: SwiftUI over the same Rust core. |

## Running it

### The server

```bash
# Postgres
docker compose up -d postgres

# The API, with demo data
LEOS_DATABASE_URL=postgres://leos:leos@localhost:5432/leos \
LEOS_DEV_MODE=true \
LEOS_SEED=true \
cargo run -p leos-server
```

The API listens on `http://127.0.0.1:8080` and creates its own schema on
first run. `scripts/dev-server.sh start|stop|restart|logs` wraps this for
day to day use.

### The whole flow, end to end

```bash
cargo run -p leos-cli -- demo
```

```
Leos end to end demo
====================

1. Signed in as Ana Reyes (+63 917 *** 0001)
2. Asked for a bank transfer reference: LEOS-20261112-8WCFFV
3. The bank confirmed the transfer of LC 2,000.00
4. Paid LC 250.00 to Brew and Bean by QR
5. Paid LC 500.00 cash at Sampaguita Cash Point, voucher code U2UA2PFB
6. Redeemed the voucher in the app
7. Sent LC 150.00 to Bello Santos, reference LEOS-20261112-5VGX6X
8. Asked for LC 500.00 in cash, reference LEOS-20261112-N6EGN6
9. Collected the cash at the agent

Final balance: LC 1,600.00
```

Every agent, merchant and bank in that run is simulated by `leos-cli`. No
money moves anywhere.

### The Android app

See [android/README.md](android/README.md). In short:

```bash
cd android
./gradlew :app:assembleDebug
```

The Gradle build compiles the Rust core for every ABI with the NDK and
packages it into the APK, so there is no separate step.

### The iOS app

See [ios/README.md](ios/README.md). It needs macOS with Xcode, which this
repository was not built on, so the Swift sources are provided complete but
unverified by a compiler.

## The money flows

**Person to person.** The sender's number is normalized to E.164, the
recipient is looked up, the limits are checked, and a journal is posted:
the sender's account is debited and the recipient's is credited. Money
never stops in between.

**Merchant QR pay.** The QR is a QR Ph payload with a CRC16 checksum. It is
decoded *on the phone*, and the checksum is checked there, before anything
is sent. The server checks it again.

**Cash in.** The customer pays an agent and receives a voucher code. The
agent's terminal issues the voucher; the customer enters the code in the
app and the wallet is credited. The voucher works once.

**Cash out.** The app reserves the money, takes the fee and gives the
customer a reference. The agent's terminal pays the cash and confirms the
reference, which settles the reservation.

**Bank top up.** The app gives a reference and the customer transfers from
their bank quoting it. The money is credited when the bank confirms the
transfer, not before.

## How the money is kept

Balances are not a number that gets updated. Every movement is a *journal*
of postings that has to balance, and a balance is the sum of the postings
against an account:

```
transfer LC 150 from Ana to Bello

  debit   user:usr_ana      150.00
  credit  user:usr_bello    150.00
```

The ledger refuses to post a journal that does not balance, the database
refuses to let a customer balance go negative, and the integration tests
check after every flow that every journal in the database still balances.

## The currency

The wallet holds **Leos Credits**, code `LCO`, symbol `LC`. It is not an
ISO 4217 currency and it exists nowhere but in this project.

That is a deliberate choice rather than a cosmetic one:

* a balance of `LC 12,345.67` cannot be read as pesos by anyone, which
  removes any chance of the simulation being mistaken for a live wallet
* there is no risk of a stray transfer reaching a real account, because
  there is no such thing as an LC account at any institution
* the mechanics are untouched. Fees, limits, cash agents, bank top ups,
  merchant settlement and the double-entry ledger all behave exactly as
  they would with a real currency, because those are the parts worth
  learning

The currency lives in one place, `crates/leos-core/src/currency.rs`, and
the amount parser still accepts a `₱` or `PHP` prefix on input, so pasting
in a peso figure does not fail. It is never displayed.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the details, and
[docs/SECURITY.md](docs/SECURITY.md) for how the PIN and the session are
protected.

## Tests

```bash
# The rules: money, phone numbers, the QR codec, the PIN derivation
cargo test -p leos-core

# The boundary
cargo test -p leos-ffi

# The whole API against a real database
docker compose up -d postgres
LEOS_TEST_DATABASE_URL=postgres://leos:leos@localhost:5432/leos \
    cargo test -p leos-server
```

The server tests start the API in process on a random port and drive it
over HTTP: registration, sign in, top up, transfer, replayed transfers,
vouchers, cash out, QR payments, tampered QR codes and sessions that stop
working after sign out.

## What is not finished

This is a working wallet, not a licensed one, and not one that could ever
hold real money as it stands. Before it could move anything real, it
would need:

* an SMS gateway for the one-time passcode, and a way to charge for it
* a real payment acquirer behind the merchant QR pay, and a real
  bank or e-money partner behind the top up
* a bug bounty and an outside audit of the ledger and the cryptography
* per-merchant settlement accounts, a cash float reconciliation job, and
  the reporting a regulator would ask for
* an in-app notification path, a linked account screen and a dispute flow

The parts that are *not* missing are the ones that make a wallet a
wallet: the ledger, the limits, the idempotency, the PIN handling, the
session handling and the payment flows. Those are real and they are
tested.
