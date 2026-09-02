//! HTTP networking: router wiring, server runtime, and the outbound HTTP client.

pub mod handler;
pub mod router;
pub(crate) mod server;
pub mod state;

use reqwest::Client;

use crate::{config::Config, error::ClientCreationError};

/// Creates a new HTTP client.
pub(crate) fn build_http_client(config: &Config) -> Result<Client, ClientCreationError> {
    let client = Client::builder()
        .user_agent(config.server.user_agent.to_string())
        .timeout(config.server.out_request_timeout)
        .build()?;

    Ok(client)
}
