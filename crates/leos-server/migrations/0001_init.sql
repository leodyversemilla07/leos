-- Leos wallet schema.
--
-- Balances are never stored as a single mutable number. Every movement of
-- money is recorded as postings against ledger accounts, and the balance of
-- an account is the sum of its postings. `account_balances` is a running
-- total kept in the same transaction as the postings, so it can carry a
-- `balance >= 0` constraint that stops the wallet from going negative even
-- if application logic were to slip.

CREATE TABLE users (
    id              TEXT PRIMARY KEY,
    phone           TEXT NOT NULL UNIQUE,
    display_name    TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    -- Verifier for the client-derived PIN secret. The server never sees a
    -- PIN: the device derives a high entropy secret from it, and the server
    -- stores a hash of that secret.
    pin_verifier    TEXT NOT NULL,
    pin_salt        TEXT NOT NULL,
    failed_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE auth_challenges (
    id          TEXT PRIMARY KEY,
    phone       TEXT NOT NULL,
    code_hash   TEXT NOT NULL,
    purpose     TEXT NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    expires_at  TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX auth_challenges_phone_idx ON auth_challenges (phone);

CREATE TABLE sessions (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    refresh_hash   TEXT NOT NULL,
    device_label   TEXT,
    expires_at     TIMESTAMPTZ NOT NULL,
    revoked_at     TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX sessions_user_idx ON sessions (user_id);

-- Ledger accounts. User wallets are created together with the user.
CREATE TABLE accounts (
    id             TEXT PRIMARY KEY,
    kind           TEXT NOT NULL,
    owner_user_id  TEXT REFERENCES users (id) ON DELETE CASCADE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX accounts_owner_idx ON accounts (owner_user_id);

-- A journal groups the postings of one transaction.
CREATE TABLE journals (
    id          TEXT PRIMARY KEY,
    tx_type     TEXT NOT NULL,
    status      TEXT NOT NULL,
    reference   TEXT NOT NULL UNIQUE,
    memo        TEXT,
    metadata    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX journals_created_idx ON journals (created_at DESC);

CREATE TABLE postings (
    id          BIGSERIAL PRIMARY KEY,
    journal_id  TEXT NOT NULL REFERENCES journals (id) ON DELETE CASCADE,
    account_id  TEXT NOT NULL REFERENCES accounts (id),
    direction   TEXT NOT NULL CHECK (direction IN ('debit', 'credit')),
    amount      BIGINT NOT NULL CHECK (amount > 0),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX postings_account_idx ON postings (account_id);
CREATE INDEX postings_journal_idx ON postings (journal_id);

-- Running balance per account, maintained in the same transaction as the
-- postings it summarises.
--
-- A customer wallet can never go negative: that is the invariant that keeps
-- Leos solvent, and the database enforces it. System accounts are a
-- different kind of thing. `cash:bank` and `cash:agents` hold money that
-- belongs to the platform rather than to a customer, so their balances are
-- allowed to run negative and mean "the platform holds this much".
CREATE TABLE account_balances (
    account_id      TEXT PRIMARY KEY REFERENCES accounts (id),
    balance         BIGINT NOT NULL,
    allow_negative  BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT account_balances_balance_check CHECK (allow_negative OR balance >= 0)
);

-- Makes a retried request safe: the first request that claims a key stores
-- its response, later requests replay it.
CREATE TABLE idempotency_keys (
    key         TEXT NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    endpoint    TEXT NOT NULL,
    response    JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, endpoint, key)
);

-- Partner agents that hold cash and can pay in or pay out.
CREATE TABLE agents (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    address     TEXT NOT NULL,
    cash_balance BIGINT NOT NULL DEFAULT 0 CHECK (cash_balance >= 0),
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Merchants that accept QR payments.
CREATE TABLE merchants (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    city        TEXT NOT NULL,
    qr_payload  TEXT NOT NULL,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A pending top-up from a bank transfer, matched by reference number.
CREATE TABLE topups (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    reference   TEXT NOT NULL UNIQUE,
    amount      BIGINT NOT NULL CHECK (amount > 0),
    status      TEXT NOT NULL DEFAULT 'pending',
    bank_code   TEXT NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    credited_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX topups_user_idx ON topups (user_id, created_at DESC);

-- Cash-in vouchers issued by agents and redeemed by the app.
CREATE TABLE vouchers (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents (id),
    user_id     TEXT REFERENCES users (id),
    reference   TEXT NOT NULL UNIQUE,
    amount      BIGINT NOT NULL CHECK (amount > 0),
    status      TEXT NOT NULL DEFAULT 'issued',
    redeemed_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A cash-out that has been authorised by the wallet and is waiting to be
-- handed over as cash by an agent.
CREATE TABLE payouts (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    reference    TEXT NOT NULL UNIQUE,
    amount       BIGINT NOT NULL CHECK (amount > 0),
    fee          BIGINT NOT NULL DEFAULT 0,
    status       TEXT NOT NULL DEFAULT 'pending',
    agent_id     TEXT REFERENCES agents (id),
    completed_at TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX payouts_user_idx ON payouts (user_id, created_at DESC);

-- System accounts the ledger posts against.
INSERT INTO accounts (id, kind) VALUES
    ('cash:agents', 'agent_cash'),
    ('cash:bank', 'bank_funding'),
    ('fee:cash_out', 'platform_fee'),
    ('fee:transfer', 'platform_fee'),
    ('fee:merchant', 'platform_fee'),
    ('suspense:cash_out', 'suspense'),
    ('platform:capital', 'suspense'),
    ('merchant:settlement', 'merchant');
