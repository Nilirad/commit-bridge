# Relay Server

Triggering a repository workflow
in response to a commit on a different repository
is not a trivial problem.
This is particularly useful 
for projects that have git dependencies.
Triggering a CI workflow
when a git dependency gets updated
is important for detecting breaking changes as soon as possible.

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
Clone this repository:

```shell
git clone https://github.com/Nilirad/relay.git
```

Follow the instructions in the **"Setup"** section,
then run the server
(unless you already deployed a container):

```shell
cargo run --release
```

**Populate the database.**
Populate the database with the subscriptions you need.
Usage of the Scalar UI,
accessible by navigating to `/scalar` on your server
(e.g., http://localhost:3000/scalar),
is preferred.
The `curl` command below is left as an example
on how to subscribe to a branch.

```shell
curl -X POST http://localhost:3000/subscribers \
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
create your `.env` file by copying the example:

```shell
cp .env.example .env
```

Then,
edit the `.env` file to add your GitHub App's client id (`GH_CLIENT_ID`)
and prepare the necessary paths for your GitHub App private key.
Finally,
follow one of the three options below.

### Docker deployment

For containerized deployment,
use the provided `docker-compose.yaml.example`:

```shell
cp docker-compose.yaml.example docker-compose.yaml
```

Ensure the path to your GitHub App private key
is correctly mapped in `docker-compose.yaml`,
then build and start the container:

```shell
docker-compose up -d
```

### Nix flake

[`Nix`] is recommended
to set up the development environment.

Ensure [flakes are enabled].
Just run `nix develop`
to enter a shell with the required environment.
If you use [`nix-direnv`],
you can automatically enter the shell
just by entering the workspace directory.

### Manual setup

If you prefer not to use Nix or a container,
you can build and run this server
by manually configuring the environment:

- Install [Rust]
  (build-time dependency).
- Install `git`
  (runtime dependency).

<!-- LINKS -->
[`Nix`]: https://nixos.org/learn/
[`nix-direnv`]: https://github.com/nix-community/nix-direnv
[flakes are enabled]: https://nixos.wiki/wiki/Flakes
[Rust]: https://rust-lang.org/learn/get-started/

## Security

You can secure the API by requiring an API key for all sensible endpoint interactions.

1.  **Configure:** In your `.env` file, set the `RELAY__AUTH__API_KEY` environment variable to a secure value.
2.  **Authenticate:** When making requests to `/subscribers` (via `curl` or other tools), include the key in the header:

    ```shell
    curl -X GET http://localhost:3000/subscribers \
      -H "X-API-KEY: YOUR_API_KEY"
    ```

If `RELAY__AUTH__API_KEY` is not set in your environment, authentication is disabled, allowing unrestricted access to these endpoints.

## License

This repository is dual-licensed under the following,
unless otherwise noted:

- [MIT LICENSE][mit]
- [Apache License, Version 2.0][apache]

at your option.

<!-- LINKS -->

[mit]: https://opensource.org/license/mit
[apache]: https://www.apache.org/licenses/LICENSE-2.0
