use anyhow::Result;
use rustls::{ClientConfig, RootCertStore, pki_types::CertificateDer};
use tokio_postgres_rustls::MakeRustlsConnect;

pub fn init() {
    let disable_colors = ["1", "true"].contains(
        &std::env::var("DISABLE_COLORS")
            .unwrap_or_else(|_| String::new())
            .to_lowercase()
            .as_str(),
    );
    owo_colors::set_override(!disable_colors);

    install_rustls_provider();
}

pub fn install_rustls_provider() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("install aws-lc-rs provider");
}

pub fn signal_ready() {
    std::fs::write("/etc/ready", b"ready").expect("could not write /etc/ready");
}

pub fn make_rustls(certs: Vec<CertificateDer<'_>>) -> Result<MakeRustlsConnect> {
    let mut roots = RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
        roots.add(cert).unwrap();
    }
    for cert in certs {
        roots.add(cert)?;
    }
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(MakeRustlsConnect::new(config))
}
