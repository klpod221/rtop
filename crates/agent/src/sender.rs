//! HTTP sender with retry, exponential back-off, optional gzip, and custom TLS.

use anyhow::{bail, Context, Result};
use reqwest::{Client, ClientBuilder};
use tokio::time::{sleep, Duration};
use tracing::warn;

use app_config::ServerConfig;

pub struct Sender {
    client: Client,
    endpoint: String,
    auth_header: String,
    auth_token: String,
    retry_count: u32,
    retry_delay: Duration,
}

impl Sender {
    /// Constructs a `Sender` from `ServerConfig`.
    pub fn new(cfg: &ServerConfig) -> Result<Self> {
        let mut builder = ClientBuilder::new()
            .timeout(Duration::from_secs(cfg.timeout_seconds))
            .danger_accept_invalid_certs(cfg.tls_skip_verify);

        if !cfg.tls_ca_cert.is_empty() {
            let pem = std::fs::read(&cfg.tls_ca_cert)
                .with_context(|| format!("reading CA cert {}", cfg.tls_ca_cert))?;
            let cert = reqwest::Certificate::from_pem(&pem)
                .context("parsing CA cert")?;
            builder = builder.add_root_certificate(cert);
        }

        let client = builder.build().context("building HTTP client")?;

        Ok(Self {
            client,
            endpoint: cfg.endpoint.clone(),
            auth_header: cfg.auth_header.clone(),
            auth_token: cfg.auth_token.clone(),
            retry_count: cfg.retry_count,
            retry_delay: Duration::from_secs(cfg.retry_delay_seconds),
        })
    }

    /// POSTs `body` to the configured endpoint with retry and backoff.
    pub async fn send(&self, body: &[u8]) -> Result<()> {
        let mut delay = self.retry_delay;
        let attempts = self.retry_count + 1;

        for attempt in 1..=attempts {
            match self.do_send(body).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt == attempts {
                        bail!("all {attempts} attempts failed: {e}");
                    }
                    warn!("send attempt {attempt}/{attempts} failed: {e} — retrying in {delay:?}");
                    sleep(delay).await;
                    delay *= 2; // exponential back-off
                }
            }
        }
        unreachable!()
    }

    async fn do_send(&self, body: &[u8]) -> Result<()> {
        let mut req = self.client.post(&self.endpoint)
            .header("Content-Type", "application/json");

        if !self.auth_token.is_empty() {
            req = req.header(&self.auth_header, &self.auth_token);
        }

        let resp = req.body(body.to_vec()).send().await
            .context("HTTP POST")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("server returned {status}: {text}");
        }
        Ok(())
    }
}
