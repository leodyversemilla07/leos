# Architecture

## The shape of it

Leos has three moving parts, and the line between them is drawn around
*who is allowed to decide what a balance is*.

**The server owns the money.** It is the only component that can change a
balance, and it does so by posting a balanced journal. The mobile app asks
questions and makes requests; it never keeps a balance of its own that
could disagree with the server.

**The phone owns the secrets.** The PIN is stretched into a key on the
device, and the session is kept in the platform keystore. The phone can
decide that a person is who they say they are, but it cannot decide that
they have money.

**The core is shared.** `leos-core` holds the rules that both sides must
agree on: what a peso amount is, what a Philippine phone number looks like,
what a valid QR payload is, what the transaction limits are. If the server
and the phone disagreed about any of those, one of them would be wrong.

## The crates

```
leos-core      no I/O, no dependencies on the platform
  money.rs       pesos as integer centavos
  phone.rs       Philippine numbers in E.164 form
  qr.rs          the EMVCo / QR Ph codec and its CRC16
  pin.rs         the PIN key derivation
  policy.rs      PIN rules, limits, fees
  domain.rs      accounts, postings, journals, transactions
  error.rs       one error type with stable codes

leos-client   the on-device core
  api.rs         the HTTP client and the session
  dto.rs         the shapes the API speaks in
  ops.rs         the JSON dispatcher the mobile apps call

leos-ffi      the boundary
  lib.rs         the C ABI, panic safe
  jni.rs         the JNI entry points for Android

leos-server   the API
  ledger.rs      posting journals, balances, limits
  auth.rs        the PIN verifier, tokens, the session extractor
  idempotency.rs making a retried request safe
  routes/        one module per part of the API

leos-cli      the partners
  main.rs        a cash agent terminal, a bank, and a scripted demo
```

## Money

A peso amount is an `i64` count of centavos. Never a float, never a
decimal string that has to be parsed into one on the way in and back out on
the way out. Over the wire an amount is the string `"1234.56"`, which is
exact and readable; inside, it is `123456`.

Adding and subtracting refuse to go negative, because a balance that goes
negative is a bug that becomes a support ticket.

The currency itself is invented: **Leos Credits**, code `LCO`, symbol
`LC`, defined in `leos-core/src/currency.rs`. It exists only here, so the
simulation can never be mistaken for a wallet holding real money. The
amount parser still accepts a `₱` or `PHP` prefix on input, so a pasted
peso figure is understood, but nothing ever displays one.

The mechanics around the currency are the real ones: fees, limits, cash
agents, bank top ups, merchant settlement and a double-entry ledger. Those
do not depend on the money being real.

## The ledger

Every movement is a *journal*: a list of postings, each naming an account
and a direction, that has to balance.

```
  debit   user:usr_ana      150.00
  credit  user:usr_bello    150.00
```

The rules the ledger enforces:

* a journal with debits that do not match credits is rejected before it
  reaches the database
* a customer balance may never go negative, and the database enforces that
  with a check constraint as well
* the balance rows are locked in a fixed order inside a transaction, so two
  transfers that cross over each other cannot deadlock
* a posting is never edited or deleted; a mistake is corrected with a
  reversing journal

System accounts are a different kind of thing. `cash:bank` and
`cash:agents` hold money that belongs to the platform rather than to a
customer, so their balances are allowed to run negative and mean "the
platform holds this much". Only customer balances are protected.

### Accounts

| Account | Holds |
|---|---|
| `user:<id>` | what a customer owns; may never go negative |
| `cash:agents` | cash held by partner agents |
| `cash:bank` | money arriving from a bank transfer |
| `merchant:<id>` | what a merchant is owed |
| `fee:<kind>` | fees Leos collected |
| `suspense:cash_out` | cash reserved for a payout that has not happened yet |
| `platform:capital` | the platform's own capital |

## The limits

A transfer is refused when it would break any of these:

| Limit | Amount |
|---|---|
| One transfer | LC 50,000 |
| One day | LC 100,000 |
| One month | LC 500,000 |
| One cash in or cash out | LC 50,000 |
| One top up | LC 100,000 |

They live in `leos-core::policy::WalletPolicy`, so the phone can show the
same numbers the server enforces.

## Idempotency

Mobile networks retry. A transfer that is sent twice because the first
response was lost must not move money twice.

Every money moving request carries an idempotency key. The first request
that claims a key stores its response; later requests with the same key
replay that response untouched. The key is scoped to the user and the
endpoint, so one key cannot be used to skip a different operation.

The key is claimed before any work is done and the response is stored
after, both inside the same flow, so a request that fails in the middle
leaves the key claimed but unfulfilled: the client gets an honest error
rather than a silent double spend.

## Sessions

Signing in returns an access token and a refresh token. The server stores
`sha256` of each, so a leaked database does not hand over a usable
credential. Access tokens are short lived, refresh tokens are long lived
and rotated on every use, and signing out revokes the session immediately.

The access token travels as a bearer token. The refresh token stays on the
device.

## The phone number

A Philippine number is normalized to E.164 before anything else happens:
`+63`, then ten digits for a mobile number or nine for a landline. People
type `0917 123 4567`, `+639171234567`, `63 917 123 4567` and
`(0917) 123-4567`, and all of them mean the same wallet.

Wallets are registered against mobile numbers only.

## The QR code

Leos speaks the QR Ph profile of the EMVCo merchant presented standard:
the country code in tag `58`, the merchant name in `59`, the city in `60`,
and a CRC16-CCITT-FALSE checksum in `63`. Plain EMVCo payloads are also
accepted, where the name is in `57` and the city in `58`.

The checksum is verified twice: on the phone, so a code that is not a
merchant QR never reaches the network, and on the server, because the
server will not take a client's word for it.

## The boundary

The mobile apps and the Rust core talk over a single JSON entry point:

```json
{"op": "transfer", "to_phone": "+639170000002", "amount": "150.00", "pin_secret": "…"}
```

```json
{"ok": true, "data": {"id": "txn_…", "amount": "150.00", "reference": "LEOS-20261112-CJWNK3"}}
```

or

```json
{"ok": false, "error": {"code": "insufficient_funds", "message": "not enough balance", "retryable": false}}
```

One entry point means no generated bindings, no version skew between the
mobile build and the core, and no need to touch the mobile build when an
operation is added. The error codes are stable, so the UI can react to
`insufficient_funds` without matching on a sentence.

Every call is panic safe: a panic in the Rust core is caught at the
boundary and turned into an error response instead of unwinding into the
app.

## The on-device core

The Rust core runs inside the app, not beside it. That is what lets the
things that matter stay in one place:

* the PIN derivation, so the PIN never crosses the network
* the QR decoder, so a payload is checked before it is sent
* the amount parsing, so the phone and the server agree on what `150.00`
  means
* the session, so the token handling is the same on both platforms

Android reaches it through JNI; iOS reaches it through a C ABI and a
bridging header. Both are generated by hand and are a handful of
functions, which is why neither platform needs a code generator.
