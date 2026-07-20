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
    /// Gix repository handle.
    #[allow(dead_code)]
    repo: std::sync::Mutex<gix::Repository>,
    /// Timeout duration for outgoing Git requests.
    timeout: std::time::Duration,
}

impl MainGitFetcher {
    /// Creates a new `MainGitFetcher` with the given gix repository.
    pub fn new(repo: gix::Repository, timeout: std::time::Duration) -> Self {
        Self {
            repo: std::sync::Mutex::new(repo),
            timeout,
        }
    }
}

#[async_trait]
impl GitFetcher for MainGitFetcher {
    async fn get_latest_hash(
        &self,
        repo_url: &str,
        branch: &str,
    ) -> Result<CommitHash, CommitHashError> {
        let repo_url = repo_url.to_string();
        let branch = branch.to_string();
        let timeout = self.timeout;
        let repo = {
            let guard = self.repo.lock().unwrap();
            guard.clone()
        };

        let fetch_task = async move {
            let mut remote = repo.remote_at(repo_url)?;
            remote.replace_refspecs(Some(branch.as_str()), Direction::Fetch)?;
            let connection = remote.connect(Direction::Fetch)?;
            let (ref_map, _) =
                connection.ref_map(Discard, gix::remote::ref_map::Options::default())?;

            let target_suffix = format!("/{}", branch);
            ref_map
                .remote_refs
                .iter()
                .find(|r| {
                    let (name, _, _) = r.unpack();
                    name == branch.as_bytes() || name.ends_with(target_suffix.as_bytes())
                })
                .ok_or_else(|| CommitHashError::Git("Branch not found".to_string()))?
                .unpack()
                .1
                .map(|id| id.to_string())
                .ok_or_else(|| CommitHashError::Git("Hash not found for branch".to_string()))
        };

        match tokio::time::timeout(timeout, fetch_task).await {
            Ok(Ok(hash)) => Ok(CommitHash::new(hash)?),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CommitHashError::UnexpectedStatus(
                "Gix operation timed out".to_string(),
            )),
        }
    }
}
