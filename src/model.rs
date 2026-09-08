//! Data structures representing items stored in database.
//!
//! The `Create_` `struct`s represent the payload
//! to create the corresponding row.

use crate::domain::{BranchName, CommitHash, EventType, RepoUrl, TargetRepo};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Represents a row in the `branches` table.
#[derive(Debug, Serialize, Deserialize, FromRow, JsonSchema)]
pub struct Branch {
    /// Unique database primary key.
    pub id: i64,

    /// Full HTTPS URL of the monitored git repository.
    pub repo_url: RepoUrl,

    /// Name of the git branch to poll.
    pub name: BranchName,

    /// SHA of the latest commit polled.
    ///
    /// `None` if the branch has not been processed.
    pub last_commit_hash: Option<CommitHash>,

    /// Timestamp when the record was created.
    pub created_at: DateTime<Utc>,

    /// Timestamp when the record was updated.
    pub updated_at: DateTime<Utc>,
}

/// Represents a row in the `subscriptions` table.
#[derive(Debug, Serialize, Deserialize, FromRow, JsonSchema, Clone)]
pub struct Subscription {
    /// Unique database primary key.
    pub id: i64,

    /// Foreign key to [`Branch::id`].
    pub branch_id: i64,

    /// The repository whose workflow needs to be triggered.
    pub target_repo: TargetRepo,

    /// Identifies the specific [`repository_dispatch`] event.
    ///
    /// The values must contain at most 100 characters.
    ///
    /// <!-- LINKS -->
    /// [`repository_dispatch`]: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#repository_dispatch
    pub event_type: EventType,

    /// Allows authenticating as a [GitHub App installation][gh_app_auth].
    ///
    /// <!-- LINKS -->
    /// [gh_app_auth]: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-as-a-github-app-installation
    pub gh_app_installation_id: i64,

    /// Timestamp when the record was created.
    pub created_at: DateTime<Utc>,

    /// Timestamp when the record was updated.
    pub updated_at: DateTime<Utc>,
}

/// Represents branch information for a subscription.
#[derive(Debug, Serialize, JsonSchema, Clone)]
pub struct SourceBranchInfo {
    /// Full HTTPS URL of the monitored git repository.
    pub repo_url: RepoUrl,
    /// Name of the git branch to poll.
    pub name: BranchName,
}

/// Combined subscription and branch information.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SubscriptionWithBranch {
    /// The subscription data.
    #[serde(flatten)]
    pub subscription: Subscription,
    /// The source branch info.
    pub source_branch: SourceBranchInfo,
}

/// Holds payload data for the creation of a [`Subscription`].
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateSubscription {
    /// Full HTTPS URL of the monitored git repository.
    pub source_repo_url: RepoUrl,

    /// Name of the git branch to poll.
    pub source_branch_name: BranchName,

    /// The repository whose workflow needs to be triggered.
    pub target_repo: TargetRepo,

    /// The `repository_dispatch` event type (at most 100 characters).
    pub event_type: EventType,

    /// The GitHub App installation used to authenticate the dispatch.
    pub gh_app_installation_id: i64,
}

/// Holds payload data for the update of a [`Subscription`].
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSubscription {
    /// The repository whose workflow needs to be triggered.
    pub target_repo: Option<TargetRepo>,

    /// The `repository_dispatch` event type (at most 100 characters).
    pub event_type: Option<EventType>,

    /// The GitHub App installation used to authenticate the dispatch.
    pub gh_app_installation_id: Option<i64>,
}

/// Represents a row in the `trigger_queue` table.
#[derive(Debug, FromRow)]
pub struct TriggerQueueItem {
    /// Unique database primary key.
    pub id: i64,

    /// Foreign key to [`Branch::id`].
    pub branch_id: i64,

    /// The hash of the latest commit on the branch.
    pub new_hash: CommitHash,

    /// The repository whose workflow needs to be triggered.
    pub target_repo: TargetRepo,

    /// The `repository_dispatch` event type (at most 100 characters).
    pub event_type: EventType,

    /// The GitHub App installation used to authenticate the dispatch.
    pub gh_app_installation_id: i64,

    /// Number of times the task has been attempted.
    pub retry_count: i64,

    /// Serialized OpenTelemetry span context.
    pub span_context: Option<String>,
}
