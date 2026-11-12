//! The currency the wallet holds.
//!
//! Leos is a simulation, so it does not hold Philippine pesos. It holds
//! *Leos Credits*, a currency that exists only inside this project. Using a
//! made up currency is deliberate: there is no way to read a balance and
//! mistake it for real money, and nothing here touches a payment network
//! or a bank account.
//!
//! The mechanics around the currency are the real mechanics of an e-wallet:
//! fees, limits, cash agents, bank top ups and merchant settlement. Those
//! are the parts worth learning, and they do not depend on the money being
//! real.

/// A currency the wallet can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Currency {
    /// The three letter code, as used by the API and the QR codes.
    pub code: &'static str,
    /// The name of the currency.
    pub name: &'static str,
    /// The symbol shown next to an amount.
    pub symbol: &'static str,
    /// The numeric code, as used in a QR code.
    pub numeric: &'static str,
}

/// The currency Leos holds.
pub const LEOS_CREDITS: Currency = Currency {
    code: "LCO",
    name: "Leos Credits",
    symbol: "LC",
    numeric: "980",
};

/// Formats an amount with the currency symbol, e.g. `LC 1,234.56`.
pub fn format(amount: crate::money::Money) -> String {
    format!(
        "{} {}",
        LEOS_CREDITS.symbol,
        amount.with_thousands_separator()
    )
}

/// The notice that says what this is.
pub const SIMULATION_NOTICE: &str = "Leos is a simulation. It holds no real money, makes no real payments, and is not a \
     registered financial service.";

/// The short notice for the interface.
pub const SIMULATION_BADGE: &str = "Simulation — no real money";
