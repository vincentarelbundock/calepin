use std::time::Duration;

pub fn timeout_agent(timeout: Duration) -> ureq::Agent {
    use ureq::{Agent, tls::TlsConfig};
    Agent::config_builder()
        .tls_config(TlsConfig::builder()
            .root_certs(root_certs())
            .build())
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

/// Verify servers against the operating system's trust store, so roots
/// installed locally (corporate CAs, TLS-intercepting proxies) are honored,
/// falling back to the bundled Mozilla roots when that store holds no
/// certificates. On Linux and the BSDs the platform verifier reads the system
/// CA bundle and refuses to build a verifier from an empty one, so without
/// the fallback a machine lacking a ca-certificates package (minimal
/// containers, typically) would fail every HTTPS request.
#[cfg(all(unix, not(target_os = "android"), not(target_vendor = "apple")))]
fn root_certs() -> ureq::tls::RootCerts {
    use std::sync::OnceLock;
    static ROOTS: OnceLock<ureq::tls::RootCerts> = OnceLock::new();
    ROOTS
        .get_or_init(|| {
            if rustls_native_certs::load_native_certs().certs.is_empty() {
                cwarn!(
                    "no CA certificates in the system trust store; \
                     falling back to the bundled Mozilla roots \
                     (install your distribution's ca-certificates package to use the system store)"
                );
                ureq::tls::RootCerts::WebPki
            } else {
                ureq::tls::RootCerts::PlatformVerifier
            }
        })
        .clone()
}

#[cfg(not(all(unix, not(target_os = "android"), not(target_vendor = "apple"))))]
fn root_certs() -> ureq::tls::RootCerts {
    ureq::tls::RootCerts::PlatformVerifier
}
