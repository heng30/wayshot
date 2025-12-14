#![windows_subsystem = "windows"]

#[tokio::main]
async fn main() {
    extern crate wayshot;

    rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::ring::default_provider().into(),
    )
    .expect("failed to set crypto provider");

    wayshot::desktop_main().await;
}
