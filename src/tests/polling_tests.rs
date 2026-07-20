use crate::{domain::CommitHash, polling::git::GitFetcher, test_utils::MockGitFetcher};

#[tokio::test]
async fn test_mock_git_fetcher() {
    let mock_hash =
        CommitHash::new("1234567890abcdef1234567890abcdef12345678".to_string()).unwrap();
    let fetcher = MockGitFetcher {
        hash: mock_hash.clone(),
    };

    let hash = fetcher.get_latest_hash("url", "branch").await.unwrap();
    assert_eq!(hash, mock_hash);
}
