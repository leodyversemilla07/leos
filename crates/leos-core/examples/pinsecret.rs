fn main() {
    let pin = std::env::args().nth(1).unwrap_or_else(|| "4821".into());
    let device = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "leos-demo-device-secret-000001".into());
    let salt = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "leos-demo-salt-0001".into());
    println!(
        "{}",
        leos_core::pin::derive_secret(&pin, &device, &salt).unwrap()
    );
}
