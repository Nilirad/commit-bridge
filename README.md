<h1 align="center">CommitBridge</h1>

<p align="center">
  <em>Seamless workflow dispatch for remote git dependencies.</em>
</p>

<p align="center">
  <a href="https://github.com/Nilirad/commit-bridge/actions/workflows/ci.yml"><img src="https://github.com/Nilirad/commit-bridge/actions/workflows/ci.yml/badge.svg" alt="CI Status"></a>&nbsp;
  <a href="https://github.com/Nilirad/commit-bridge/actions/workflows/deny.yml"><img src="https://github.com/Nilirad/commit-bridge/actions/workflows/deny.yml/badge.svg" alt="Security Audit"></a>&nbsp;
  <a href="https://github.com/Nilirad/commit-bridge/blob/main/Cargo.toml"><img src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg" alt="Dual License"></a>
</p>

<p align="center">
  <a href="https://crates.io/crates/commit-bridge"><img src="https://img.shields.io/crates/v/commit-bridge.svg" alt="Crates.io Version"></a>&nbsp;
  <a href="https://docs.rs/commit-bridge"><img src="https://docs.rs/commit-bridge/badge.svg" alt="Docs.rs Status"></a>&nbsp;
  <a href="https://hub.docker.com/r/Nilirad/commit-bridge"><img src="https://img.shields.io/docker/v/Nilirad/commit-bridge?sort=semver&logo=docker" alt="Docker Image"></a>
</p>

---

Triggering a repository workflow
in response to a commit on a different repository is not a trivial problem.
This is particularly useful for projects that have git dependencies,
where breaking changes need to be detected as soon as possible.

This project attempts to solve this problem
by providing a server that acts as an intermediary
between the repositories containing the dependencies
and the repositories that need those dependencies.
This project uses
[`axum`] to handle incoming requests,
[`reqwest`] to send requests to the GitHub API,
[`git ls-remote`] to check the last commit on a remote branch,
and [`sqlx`] connected to a SQLite database to hold state.

<!-- LINKS -->
[`axum`]: https://docs.rs/axum/latest/axum/
[`reqwest`]: https://docs.rs/reqwest/latest/reqwest/
[`git ls-remote`]: https://git-scm.com/docs/git-ls-remote
[`sqlx`]: https://docs.rs/sqlx/latest/sqlx/

## Usage

**Setup GitHub App.**
To authorize the server to trigger a workflow in your repository,
set up and install a GitHub App
with `Contents` set to `Read and write` permissions.
Then,
annotate your client id
and download your private PEM key.

**Setup workflow on target repository.**
Set up your GitHub Actions workflow
to be triggered by a `repository_dispatch` event:

```yaml
on:
  repository_dispatch:
    types: [EVENT_TYPE]
```

`EVENT_TYPE` is a string containing up to 100 characters.
It is used to distinguish the event
from other `repository_dispatch` events.

**Run the server.**
Follow the instructions in the **"Setup"** and **"Security"** sections
to install and run the server (such as via Docker, Cargo, or Nix).

**Populate the database.**
Populate the database with the subscriptions you need.
Usage of the Scalar UI,
accessible by navigating to `/scalar` on your server
(e.g., http://localhost:3000/scalar),
is preferred.
The `curl` command below is left as an example
on how to subscribe to a branch.

```shell
curl -X POST http://localhost:3000/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "source_repo_url": "SOURCE_REPOSITORY",
    "source_branch_name": "BRANCH_NAME",
    "target_repo": "YOUR_REPOSITORY",
    "event_type": "EVENT_TYPE",
    "gh_app_installation_id": YOUR_INSTALLATION_ID
  }'
```

Make sure that `EVENT_TYPE` is the same
as the one defined in the workflow.

**Wait for changes.**
At this point,
the server is ready to listen to the source repository
and trigger your workflow shortly after a new commit is pushed
(about 5 minutes or less).

## Setup

First,
create a `.env` file with your GitHub App configuration
or set the corresponding environment variables:

- `CBRIDGE__AUTH__CLIENT_ID`: Your GitHub App's Client ID.
- `CBRIDGE__AUTH__PEM_PATH`: The path to your GitHub App private key.

If you have cloned the repository,
you can copy the example file:

```shell
cp .env.example .env
```

Then,
follow one of the installation options below.

> [!warning]
> `.env` files are only suggested for development environments.
> In production environments,
> storing the `.env` file beside the server might introduce security risks.
> Therefore, always prefer storing configuration in system environment variables,
> or keeping the `.env` file separated from the server.

### Docker deployment

For containerized deployment,
you can use the pre-built Docker image from Docker Hub.
Create a `docker-compose.yaml`
by copying the example in this repository
or using the template below:

```yaml
services:
  commit-bridge:
    image: nilirad/commit-bridge:latest
    container_name: commit-bridge-server
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - commit_bridge_data:/app/data
      - ./YOUR_PEM_FILE.pem:/app/data/YOUR_PEM_FILE.pem:ro
    env_file:
      - .env

volumes:
  commit_bridge_data:
```

Ensure that your private key file
(`./YOUR_PEM_FILE.pem` in the example above)
exists on the host machine before launching the container
to prevent Docker from auto-creating it as an empty directory.
Once the path is correctly mapped,
start the container:

```shell
docker compose up -d
```

### Cargo installation

You can install and run the server directly from [crates.io]:

```shell
cargo install commit-bridge
```

Ensure `git` is installed (runtime dependency)
and that your environment variables or `.env` are configured.
Then, launch the server:

```shell
commit-bridge
```

### Nix flake (development)

[`Nix`] is recommended
to set up the development environment.

Ensure [flakes are enabled].
Just run `nix develop`
to enter a shell with the required environment.
If you use [`nix-direnv`],
you can automatically enter the shell
just by entering the workspace directory with a terminal.

Launch the server using one of the following commands:

```shell
cargo run

cargo run --release

nix run
```

### Manual setup (from source)

If you prefer not to use Nix or a container,
you can build and run this server
by manually configuring the environment:

- Install [Rust]
  (build-time dependency).
- Install `git`
  (runtime dependency).

Then,
launch the server:

```shell
cargo run

cargo run --release
```

<!-- LINKS -->
[`Nix`]: https://nixos.org/learn/
[`nix-direnv`]: https://github.com/nix-community/nix-direnv
[flakes are enabled]: https://nixos.wiki/wiki/Flakes
[Rust]: https://rust-lang.org/learn/get-started/
[crates.io]: https://crates.io/crates/commit-bridge

## Security

By default, this server mandates authentication for all `/subscriptions` endpoints.

1. **Configure:** Set the `CBRIDGE__AUTH__API_KEY` environment variable to a secure value in your `.env` file.
2. **Authenticate:** Include the key in the `X-API-KEY` header for all requests:

    ```shell
    curl -X GET http://localhost:3000/subscriptions \
      -H "X-API-KEY: YOUR_API_KEY"
    ```

### API Key Security

To mitigate timing attacks,
the server uses constant-time comparison for API keys.
Note that while this protects against key content discovery,
an attacker may still be able to infer the length of the API key
by measuring response times.
For maximum security,
ensure that your `CBRIDGE__AUTH__API_KEY` is long
and generated using a cryptographically secure random source.

### Disabling Authentication (Not Recommended)

> [!warning]
> Enabling this flag allows unrestricted access
> to endpoints capable of triggering remote GitHub workflows.
> Use only in trusted development environments.

If you require an unauthenticated setup for rapid local prototyping,
you can explicitly opt-in by setting the following environment variable:

```text
CBRIDGE__AUTH__ALLOW_UNAUTHENTICATED=true
```

## License

This repository is dual-licensed under the following,
unless otherwise noted:

- [MIT LICENSE][mit]
- [Apache License, Version 2.0][apache]

at your option.

<!-- LINKS -->

[mit]: https://opensource.org/license/mit
[apache]: https://www.apache.org/licenses/LICENSE-2.0
