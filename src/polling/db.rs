//! Database operations for the polling engine.

use futures::{StreamExt, stream};
use sqlx::SqlitePool;
use tracing::warn;

use crate::{
    context::SharedContext, error::CommitHashError, model::Branch, polling::branch::BranchInfo,
};

/// Gathers stored branches that need to be updated.
pub(super) async fn gather_updated_branches(
    ctx: &SharedContext,
) -> Result<Vec<BranchInfo>, sqlx::Error> {
    let branch_results = stream::iter(collect_branches(&ctx.db_pool).await?)
        .map(|b| BranchInfo::new(b, ctx.git_fetcher.as_ref()))
        .buffer_unordered(ctx.config.database.polling_db_buffer_size)
        .collect::<Vec<Result<BranchInfo, CommitHashError>>>()
        .await;

    let errs = branch_results.iter().filter_map(|res| res.as_ref().err());
    for e in errs {
        warn!("{e}");
    }

    let updated_branches = branch_results
        .into_iter()
        .filter_map(|res| res.ok())
        .filter(BranchInfo::has_updated)
        .collect();
    Ok(updated_branches)
}

/// Collects all branch rows.
async fn collect_branches(pool: &SqlitePool) -> Result<Vec<Branch>, sqlx::Error> {
    let branches = sqlx::query_as::<_, Branch>("SELECT * FROM branches")
        .fetch_all(pool)
        .await?;

    Ok(branches)
}

/// Writes the updated branch hash to the row.
pub(super) async fn write_db<'e, E>(
    branch_info: &BranchInfo,
    executor: E,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query!(
        "UPDATE branches SET last_commit_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        branch_info.latest_hash,
        branch_info.branch.id
    )
    .execute(executor)
    .await?;

    Ok(())
}
