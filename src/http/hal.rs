//! HAL representations of subscriptions, served by the HTTP API.
//!
//! These types describe API responses only (they are never stored),
//! so they live in the HTTP layer instead of `model.rs`,
//! which holds the persistence and shared models they wrap.

use schemars::JsonSchema;
use serde::Serialize;

use crate::model::{SourceBranchInfo, Subscription, SubscriptionWithBranch};

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
    /// The source repository and branch.
    pub source_branch: SourceBranchInfo,
    /// HAL links.
    #[serde(rename = "_links")]
    pub links: SubscriptionLinks,
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

/// Maps a [`SubscriptionWithBranch`] to its HAL representation.
pub(super) fn map_to_hal(sub_with_branch: SubscriptionWithBranch) -> SubscriptionHal {
    let id = sub_with_branch.subscription.id;
    SubscriptionHal {
        subscription: sub_with_branch.subscription,
        source_branch: sub_with_branch.source_branch,
        links: SubscriptionLinks {
            self_link: HalLink {
                href: format!("/subscriptions/{}", id),
            },
            update: HalLink {
                href: format!("/subscriptions/{}", id),
            },
            delete: HalLink {
                href: format!("/subscriptions/{}", id),
            },
        },
    }
}
