//! Operations to fetch and extract git branch data from remote repositories.

use crate::{domain::CommitHash, error::CommitHashError};
use async_trait::async_trait;
use gix::progress::Discard;
use gix::remote::Direction;

/// Allows running git commands.
#[async_trait]
pub trait GitFetcher: Send + Sync {
    /// Returns the latest hash of a git branch.
    async fn get_latest_hash(
        &self,
        repo_url: &str,
        branch: &str,
    ) -> Result<CommitHash, CommitHashError>;
}

/// Runs git commands.
pub struct MainGitFetcher {
    /// Gix thread-safe repository handle.
    repo: gix::ThreadSafeRepository,
    /// Timeout duration for outgoing Git requests.
    timeout: std::time::Duration,
}

impl MainGitFetcher {
    /// Creates a new `MainGitFetcher` with the given gix repository.
    pub fn new(mut repo: gix::Repository, timeout: std::time::Duration) -> Self {
        let timeout_secs = timeout.as_secs().max(1).to_string();

        let mut config = repo.config_snapshot_mut();
        let _ = config.set_raw_value("http.lowSpeedLimit", "1");
        let _ = config.set_raw_value("http.lowSpeedTime", timeout_secs.as_str());
        let _ = config.set_raw_value("http.connectTimeout", timeout_secs.as_str());
        let _ = config.commit();

        Self {
            repo: repo.into_sync(),
            timeout,
        }
    }
}

#[async_trait]
impl GitFetcher for MainGitFetcher {
    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", repo_url = %repo_url, branch = %branch)
    )]
    async fn get_latest_hash(
        &self,
        repo_url: &str,
        branch: &str,
    ) -> Result<CommitHash, CommitHashError> {
        let repo_url = repo_url.to_string();
        let branch = branch.to_string();
        let timeout = self.timeout;
        let thread_safe_repo = self.repo.clone();

        let fetch_task = tokio::task::spawn_blocking(move || {
            let repo = thread_safe_repo.to_thread_local();
            let mut remote = repo.remote_at(repo_url)?;
            remote = remote.with_fetch_tags(gix::remote::fetch::Tags::None);
            remote.replace_refspecs(Some(branch.as_str()), Direction::Fetch)?;
            let connection = remote.connect(Direction::Fetch)?;
            let (ref_map, _) =
                connection.ref_map(Discard, gix::remote::ref_map::Options::default())?;

            let target_head = format!("refs/heads/{}", branch);
            let target_tag = format!("refs/tags/{}", branch);
            let target_head_bytes = target_head.as_bytes();
            let target_tag_bytes = target_tag.as_bytes();
            let branch_bytes = branch.as_bytes();

            ref_map
                .remote_refs
                .iter()
                .find(|r| {
                    let (name, _, _) = r.unpack();
                    name == branch_bytes || name == target_head_bytes || name == target_tag_bytes
                })
                .ok_or_else(|| CommitHashError::Git("Branch not found".to_string()))?
                .unpack()
                .1
                .map(|id| id.to_string())
                .ok_or_else(|| CommitHashError::Git("Hash not found for branch".to_string()))
        });

        match tokio::time::timeout(timeout, fetch_task).await {
            Ok(Ok(Ok(hash))) => Ok(CommitHash::new(hash)?),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(e)) => Err(CommitHashError::UnexpectedStatus(format!(
                "Spawn blocking failed: {}",
                e
            ))),
            Err(_) => Err(CommitHashError::UnexpectedStatus(
                "Gix operation timed out".to_string(),
            )),
        }
    }
}
