//! Helpers to build GitHub API requests.

use reqwest::RequestBuilder;

use crate::config::GitHubApiConfig;

/// Returns the full API URL for `path` based on the configured base URL.
///
/// A leading `/` on `path` is optional: exactly one separator is inserted
/// between the base URL and the path.
pub(super) fn endpoint(config: &GitHubApiConfig, path: &str) -> String {
    format!(
        "{}/{}",
        config.base_url.as_str().trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Adds the GitHub API `Accept` and `X-GitHub-Api-Version` headers
/// to `request`.
pub(super) fn apply_api_headers(
    config: &GitHubApiConfig,
    request: RequestBuilder,
) -> RequestBuilder {
    request
        .header("Accept", config.accept_header.to_string())
        .header("X-GitHub-Api-Version", config.version.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::expect_used,
        clippy::todo,
        clippy::unimplemented,
        clippy::indexing_slicing
    )]

    use url::Url;

    use crate::config::GitHubApiConfig;
    use crate::domain::{AcceptHeader, ApiVersion};

    use super::{apply_api_headers, endpoint};

    fn test_config() -> GitHubApiConfig {
        GitHubApiConfig {
            base_url: Url::parse("https://api.github.com").unwrap(),
            version: ApiVersion::new("2026-03-10".to_string()).unwrap(),
            accept_header: AcceptHeader::new("application/vnd.github+json".to_string()).unwrap(),
        }
    }

    #[test]
    fn endpoint_without_trailing_slash() {
        let config = GitHubApiConfig {
            base_url: Url::parse("https://api.github.com/api/v3").unwrap(),
            ..test_config()
        };
        assert_eq!(
            endpoint(&config, "/repos/org/repo/dispatches"),
            "https://api.github.com/api/v3/repos/org/repo/dispatches"
        );
    }

    #[test]
    fn endpoint_with_trailing_slash() {
        let config = GitHubApiConfig {
            base_url: Url::parse("https://api.github.com/").unwrap(),
            ..test_config()
        };
        assert_eq!(
            endpoint(&config, "/app/installations/1/access_tokens"),
            "https://api.github.com/app/installations/1/access_tokens"
        );
    }

    #[test]
    fn endpoint_without_leading_slash() {
        let config = test_config();
        assert_eq!(
            endpoint(&config, "repos/org/repo/dispatches"),
            "https://api.github.com/repos/org/repo/dispatches"
        );
    }

    #[test]
    fn apply_api_headers_sets_expected_headers() {
        let config = test_config();
        let client = reqwest::Client::new();
        let request = apply_api_headers(&config, client.get("https://example.com"))
            .build()
            .unwrap();
        assert_eq!(
            request.headers().get("Accept").unwrap().to_str().unwrap(),
            "application/vnd.github+json"
        );
        assert_eq!(
            request
                .headers()
                .get("X-GitHub-Api-Version")
                .unwrap()
                .to_str()
                .unwrap(),
            "2026-03-10"
        );
    }
}
