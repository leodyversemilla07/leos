//! Derives the PIN secret the way the app does.
//!
//! The app stretches a PIN into a secret on the device, and only that
//! secret is sent to the server. This example does the same thing, so the
//! tools can sign in to a wallet without the app.
//!
//! ```text
//! cargo run -p leos-core --example pinsecret -- 4821
//! cargo run -p leos-core --example pinsecret -- 4821 <device secret> <salt>
//! ```

fn main() {
    let mut arguments = std::env::args().skip(1);

    let pin = arguments.next().unwrap_or_else(|| "4821".to_string());
    let device_secret = arguments
        .next()
        .unwrap_or_else(|| "leos-demo-device-secret-000001".to_string());
    let salt = arguments
        .next()
        .unwrap_or_else(|| "leos-demo-salt-0001".to_string());

    match leos_core::pin::derive_secret(&pin, &device_secret, &salt) {
        Ok(secret) => println!("{secret}"),
        Err(error) => {
            eprintln!("the secret could not be derived: {error}");
            std::process::exit(1);
        }
    }
}
