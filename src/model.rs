//! Data structures representing items stored in database.
//!
//! The `Create_` `struct`s represent the payload
//! to create the corresponding row.

// FIXME: Some docstrings have been duplicated.
// Maybe this problem can be solved by including an external Markdown file.
// For example:
//
// ```rust
// #[doc = include_str!("docs/my_struct.md")]
// pub struct MyStruct { ... }
// ```

use crate::domain::{BranchName, CommitHash, EventType, RepoUrl, TargetRepo};
use chrono::{DateTime, Utc};
use rovo::schemars::JsonSchema;
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
#[derive(Debug, Serialize, Deserialize, FromRow, JsonSchema)]
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

/// HAL links for a subscription page.
#[derive(Serialize, JsonSchema)]
pub struct SubscriptionPageLinks {
    /// Next page link.
    pub next: Option<HalLink>,
}

/// Paginated representation of subscriptions.
#[derive(Serialize, JsonSchema)]
pub struct SubscriptionPage {
    /// The subscription data.
    pub data: Vec<SubscriptionHal>,
    /// Number of elements remaining after this page.
    pub remaining_count: i64,
    /// HAL links.
    #[serde(rename = "_links")]
    pub links: SubscriptionPageLinks,
}

/// HAL link structure.
#[derive(Serialize, JsonSchema)]
pub struct HalLink {
    /// URL of the link.
    pub href: String,
}

/// HAL links for a subscription.
#[derive(Serialize, JsonSchema)]
pub struct SubscriptionLinks {
    /// Self link.
    #[serde(rename = "self")]
    pub self_link: HalLink,
    /// Update link.
    pub update: HalLink,
    /// Delete link.
    pub delete: HalLink,
}

/// HAL representation of a subscription.
#[derive(Serialize, JsonSchema)]
pub struct SubscriptionHal {
    /// The subscription data.
    #[serde(flatten)]
    pub subscription: Subscription,
    /// HAL links.
    #[serde(rename = "_links")]
    pub links: SubscriptionLinks,
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
}

/// Holds payload data for the update of a [`Subscription`].
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateSubscription {
    /// The repository whose workflow needs to be triggered.
    pub target_repo: Option<TargetRepo>,

    /// Identifies the specific [`repository_dispatch`] event.
    ///
    /// The values must contain at most 100 characters.
    ///
    /// <!-- LINKS -->
    /// [`repository_dispatch`]: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#repository_dispatch
    pub event_type: Option<EventType>,

    /// Allows authenticating as a [GitHub App installation][gh_app_auth].
    ///
    /// <!-- LINKS -->
    /// [gh_app_auth]: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-as-a-github-app-installation
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

    /// Identifies the specific [`repository_dispatch`] event.
    ///
    /// <!-- LINKS -->
    /// [`repository_dispatch`]: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#repository_dispatch
    pub event_type: EventType,

    /// Allows authenticating as a [GitHub App installation][gh_app_auth].
    ///
    /// <!-- LINKS -->
    /// [gh_app_auth]: https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-as-a-github-app-installation
    pub gh_app_installation_id: i64,

    /// Number of times the task has been attempted.
    pub retry_count: i64,
}
