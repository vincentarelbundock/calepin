use std::time::Duration;

pub fn timeout_agent(timeout: Duration) -> ureq::Agent {
    use ureq::{Agent, tls::{TlsConfig, RootCerts}};
    Agent::config_builder()
        .tls_config(TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .build())
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}
