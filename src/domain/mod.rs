//! Domain models for the application.

pub mod accept_header;
pub mod api_version;
pub mod branch_name;
pub mod commit_hash;
pub mod event_type;
pub mod non_empty_string;
pub mod repo_url;
pub mod target_repo;

pub use accept_header::AcceptHeader;
pub use api_version::ApiVersion;
pub use branch_name::BranchName;
pub use commit_hash::CommitHash;
pub use event_type::EventType;
pub use non_empty_string::NonEmptyString;
pub use repo_url::RepoUrl;
pub use target_repo::TargetRepo;

/// Derives `sqlx` trait implementations for a type that implements `TryFrom<String>`.
///
/// This macro implements `sqlx::Type`, `sqlx::Decode`, and `sqlx::Encode` for the
/// specified type, assuming it can be converted to/from a `String` and uses
/// the same underlying representation as `String` in the database.
#[macro_export]
macro_rules! derive_sqlx_traits {
    ($t:ty) => {
        impl sqlx::Type<sqlx::Sqlite> for $t {
            fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
                <String as sqlx::Type<sqlx::Sqlite>>::type_info()
            }
        }
        impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for $t {
            fn decode(
                value: sqlx::sqlite::SqliteValueRef<'r>,
            ) -> Result<Self, sqlx::error::BoxDynError> {
                let s = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
                <$t>::try_from(s).map_err(|e| Box::new(e) as sqlx::error::BoxDynError)
            }
        }
        impl sqlx::Encode<'_, sqlx::Sqlite> for $t {
            fn encode_by_ref(
                &self,
                buf: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'_>>,
            ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
                <String as sqlx::Encode<sqlx::Sqlite>>::encode_by_ref(&self.to_string(), buf)
            }
        }
    };
}
