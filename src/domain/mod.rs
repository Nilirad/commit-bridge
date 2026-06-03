//! Domain models for the application.

pub mod branch_name;
pub mod event_type;
pub mod repo_url;
pub mod target_repo;

pub use branch_name::BranchName;
pub use event_type::EventType;
pub use repo_url::RepoUrl;
pub use target_repo::TargetRepo;
