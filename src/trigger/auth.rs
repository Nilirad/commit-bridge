//! Authentication and authorization to request services.

use async_trait::async_trait;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    config::Config,
    model::Subscriber,
    trigger::error::{AuthError, RequestError},
};

/// Provides authentication functionality.
#[async_trait]
pub trait Authenticator {
    /// Requests a GitHub Installation Access Token.
    async fn request_installation_token(&self, sub: &Subscriber) -> Result<String, AuthError>;
}

/// An [`Authenticator`] for GitHub.
pub struct GitHubAuthenticator {
    /// The HTTP client to make requests to the authentication server.
    pub http_client: reqwest::Client,

    /// Application configuration.
    pub config: Config,

    /// The key to sign JWTs.
    pub encoding_key: EncodingKey,
}

#[async_trait]
impl Authenticator for GitHubAuthenticator {
    async fn request_installation_token(
        &self,
        subscriber: &Subscriber,
    ) -> Result<String, AuthError> {
        let jwt = generate_gh_jwt(&self.encoding_key, &self.config)?;
        request_iat(&self.http_client, &jwt, subscriber, &self.config).await
    }
}

/// Payload that GitHub expects in the JWT.
///
/// Read more on [GitHub's documentation][jwt_docs].
///
/// <!-- LINKS -->
/// [jwt_docs]: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app
#[derive(Debug, Serialize, Deserialize)]
struct GitHubClaims {
    /// Issued at time (UNIX time).
    iat: u64,

    /// Expiration time (UNIX time).
    exp: u64,

    /// Issuer: GitHub App's Client ID.
    iss: String,
}

/// Generates a JWT to authenticate access to the GitHub App.
///
/// Implementation based on [GitHub's documentation][jwt_docs].
///
/// <!-- LINKS -->
/// [jwt_docs]: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app
pub(super) fn generate_gh_jwt(key: &EncodingKey, config: &Config) -> Result<String, AuthError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let claims = GitHubClaims {
        iat: now - config.auth.clock_drift_buffer.as_secs(),
        exp: now + config.auth.token_validity.as_secs(),
        iss: config.auth.client_id.to_string(),
    };

    let header = Header::new(Algorithm::RS256);

    let jwt = encode(&header, &claims, key)?;
    Ok(jwt)
}

/// Requests an Installation Access Token (IAT)
/// to operate on a GitHub App installation.
///
/// Implementation based on [GitHub's documentation][iat_docs].
///
/// <!-- LINKS -->
/// [iat_docs]: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app
pub(super) async fn request_iat(
    http_client: &Client,
    jwt: &str,
    sub: &Subscriber,
    config: &Config,
) -> Result<String, AuthError> {
    #[derive(serde::Deserialize)]
    struct IatResponse {
        token: String,
    }

    let api_url = format!(
        "{}/app/installations/{}/access_tokens",
        config.github_api.base_url.as_str().trim_end_matches('/'),
        sub.gh_app_installation_id
    );
    let response = http_client
        .post(&api_url)
        .bearer_auth(jwt)
        .header("Accept", config.github_api.accept_header.to_string())
        .header(
            "X-GitHub-Api-Version",
            config.github_api.version.to_string(),
        )
        .send()
        .await?;

    if response.status().is_success() {
        let response_json = response.json::<IatResponse>().await?;
        info!("IAT received for subscriber {}", sub.target_repo);
        Ok(response_json.token)
    } else {
        Err(AuthError::Server(RequestError::Response {
            status: response.status(),
            text: response.text().await?,
        }))
    }
}
