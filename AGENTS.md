# Agent Instructions for `commit-bridge`

This file contains crucial context for AI agents working in this repository.

## Development Commands
- **Testing**: Run `cargo nextest run --locked --all-features` (CI specifically uses `nextest`; prefer this over standard `cargo test`).
- **Linting**: Run `cargo clippy --locked --all-targets --all-features -- -D warnings`.
- **Formatting**: Run `cargo fmt --all -- --check`.
- **SQLx Offline Mode**: This project uses `sqlx` with offline query checking (evidenced by the `.sqlx/` dir). Whenever you modify a database query, you **must** run `cargo sqlx prepare -- --all-targets` to update the cache.
- **Migration Creation**: Always use `cargo sqlx migrate add <name>` to generate new migrations. Manual creation of migration files is strictly prohibited to ensure proper tracking and naming conventions.
- **Destructive Schema Changes**: Dropping/renaming columns requires synchronization with `sqlx` cache.
  1. Remove code references (mark fields `#[allow(dead_code)]` if needed).
  2. Run `cargo sqlx prepare`.
  3. Apply `ALTER TABLE ... DROP COLUMN` migration.
  4. Cleanup removed fields/references.
  5. Run `cargo sqlx prepare` again.

## Environment & Setup
- **Nix First**: The project uses Nix flakes (`flake.nix`) and `direnv`. Agents should rely on `nix develop` to get the proper Rust toolchain, `sqlx-cli`, and wrapped `cargo` commands.
- **Runtime Variables**: To run the server locally, you must ensure `GH_CLIENT_ID` and `GH_APP_KEY_PATH` are set (pointing to a valid GitHub App private key).
- **External Dependencies**: The application shells out to `git ls-remote` at runtime, so `git` must be available in the environment.

## Architecture
- **Frameworks**: `axum` for HTTP, `sqlx` (SQLite) for state, `tokio` for async execution.
- **Execution Flow**: `src/main.rs` initializes an `axum` router and spawns two decoupled background `tokio` tasks:
  1. `polling/`: Periodically checks remote git repositories for updates.
  2. `trigger/`: Receives update events from the polling engine via `mpsc` channels and triggers GitHub Action workflows on target repositories.
- **Error Handling**: Use domain-specific error enums (`HandlerError`, `FatalError`) defined in `src/error.rs` using the `thiserror` crate. Ensure `IntoResponse` is implemented for any errors that bubble up to Axum handlers.
