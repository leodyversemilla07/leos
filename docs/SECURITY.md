# Security

What the wallet does to keep a customer's money safe, and what it does not
yet do.

The first thing to say is what this is not: Leos is a simulation. It holds
Leos Credits, a currency invented for this project, and it is not a
registered financial service. There are no users, no funds and no
licence. The security work below is real, and it is the work that would
carry over to a wallet that did hold something real, but the risk being
managed today is the risk of a bad implementation, not the risk of losing
somebody's money.

## The PIN never leaves the device

A four to six digit PIN is not a credential you can send over a network.
So it is not.

On the device, the PIN is stretched with Argon2id into a 32 byte secret:

```
secret = Argon2id(pin || device_secret, salt, m = 64 MiB, t = 3, p = 1)
```

* `pin` is what the user typed.
* `device_secret` is a random value kept in the Android keystore or the
  iOS keychain. It never leaves the device.
* `salt` is random per device and kept alongside it.

Only `secret` is sent to the server. The server stores a second Argon2id
hash of that secret, so:

* a stolen database does not reveal the PIN, and the verifier is expensive
  to attack offline
* a stolen verifier cannot be replayed against another device, because
  every device mixes in its own secret and salt
* the server never sees something the user would recognise as their PIN

The cost of the derivation is real: about a quarter of a second on a
mid-range handset, and about 20 MB of memory. That is deliberate. It is
the price of a PIN being safe on a phone that might be stolen.

### What the PIN authorises

The PIN is required for anything that moves money out of the wallet:
transfers, merchant payments and cash out. Reading the balance, the
history and the pending top ups needs only a session.

### Locking out

Five wrong PINs lock the wallet for fifteen minutes. The lock is counted
per account and survives a restart of the app.

## The one time passcode

Registration and PIN resets ask for a six digit code that is sent to the
mobile number. The server stores a SHA-256 hash of it, and the code:

* expires after five minutes
* can be tried five times, then the challenge is locked
* is consumed on use, so it cannot be replayed

In development the code is returned in the API response and printed to the
log. `LEOS_DEV_MODE` controls this and must not be on in production.

The real system would send the code through an SMS gateway. That is the
one part of the flow that is still a stub, and it is the weakest link in
the whole design: an SMS code is only as strong as the mobile network.
Real deployments add number verification and device binding to it.

## Sessions

Signing in returns an access token and a refresh token.

* The access token is short lived (15 minutes by default) and travels as a
  bearer token.
* The refresh token is long lived and is rotated every time it is used.
* The server stores `sha256` of both, so a database dump does not contain
  a usable credential.
* Signing out revokes the session immediately, and the token stops
  working on the next request.

On the device the session is kept in the Android keystore or the iOS
keychain, encrypted with a key that is not exportable.

## The QR code

A merchant QR is decoded and its CRC16 checksum is verified *on the
phone*. A code that is not a merchant QR never reaches the network. The
server verifies it again, because the server will not take the client's
word for it.

## What the server refuses to do

* A journal that does not balance is never posted.
* A customer balance that would go negative is refused, by the
  application and by a check constraint in the database.
* A transfer to a number that has no wallet is refused.
* A transfer to yourself is refused.
* A voucher can be redeemed once.
* A top up reference can be confirmed once.
* A request that replays an idempotency key returns the first response
  instead of doing the work again.
* The partner endpoints need a partner key that is separate from any
  customer session.

## Money in transit and at rest

The API is meant to run behind TLS. In development it runs over plain
HTTP on the loopback interface, and the Android app allows cleartext only
for the emulator's host address.

Nothing that can rebuild a PIN is written to disk. The device secret, the
salt and the session are encrypted with a key held in the platform
keystore, and the app opts out of backup so that the session does not
travel to another device.

## What is not done yet

Being honest about the gaps:

* **No audit of the cryptography.** The primitives are the right ones and
  the parameters are conservative, but nobody outside the project has
  looked at them.
* **No rate limiting at the edge.** There is a per-account lockout on the
  PIN and a limit on passcode attempts, but nothing stops a flood against
  the registration and top up endpoints.
* **No device binding.** A session can be moved to another device if
  someone copies it out of the keystore. Binding a session to a device
  key would stop that.
* **The biometric unlock is an app level gate.** The PIN secret is stored
  encrypted in the keystore, and the biometric prompt guards access to it
  inside the app, but the keystore key itself is not set to require user
  authentication. Making it auth-bound would mean the session could only
  be decrypted after a successful biometric prompt, which is the right way
  round and is not done yet.
* **No fraud checks.** There is no velocity rule, no device fingerprint,
  no behaviour model, and no way to freeze a wallet from the operations
  side.
* **The passcode is delivered by SMS.** Real deployments need a number
  verification flow and a rate limited gateway.
* **The top up and the merchant payment are simulated.** No real money
  moves, and no real acquirer or bank is connected.
* **No penetration test and no bug bounty.**
