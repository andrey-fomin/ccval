# Releasing

Releases are cut through an explicit release PR.

## What the developer does

1. Merge regular pull requests into `main`.
2. Wait for `release-plz` to open or update the release PR.
3. Review the generated version bump and `CHANGELOG.md` in that release PR.
4. Merge the release PR when you want to publish the next stable release.
5. Watch the release workflows publish GitHub artifacts and Docker Hub images.

The developer never edits the version by hand for a normal release.

## What the system does

After every push to `main`, GitHub Actions runs `release-plz`.

- It looks at commits since the last published release.
- If there are unreleased changes, it creates or updates a single release PR.
- In that release PR, it updates `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md`.
- When the release PR is merged, it creates the git tag and GitHub Release.
- After the GitHub Release is published, GitHub Actions builds release archives and SHA256 checksum files for supported platforms.
- After the release artifacts are uploaded, GitHub Actions opens or updates a PR in `andrey-fomin/homebrew-tap` with the new formula version and source archive checksum.
- Linux and macOS artifacts are built on Linux whenever possible, using cross-compilation for macOS and `aarch64-unknown-linux-gnu`.
- Windows artifacts are built on Windows runners.
- Docker images are then published to Docker Hub for that same release.

## How version bumps work

The version is chosen by `release-plz` inside the release PR.

- non-breaking commits such as `fix`, `docs`, `ci`, `chore`, `build`, `refactor`, and `test` normally produce a patch bump.
- `feat` changes normally produce a minor bump.
- breaking changes normally produce a major bump.
- this repository sets `features_always_increment_minor = true`, so `feat` still bumps the minor version while the project is in `0.x`.

The version is not finalized when a feature or fix PR is merged.
It is finalized only when the release PR is merged.

That means multiple merged PRs usually become one released version.

Example:

1. The latest release is `0.1.5`.
2. A `fix` PR is merged into `main`.
3. `release-plz` opens or updates a release PR and may propose `0.1.6`.
4. Another `fix` PR is merged. The same release PR is updated and still usually stays `0.1.6`.
5. A `feat` PR is merged before the release PR is merged. The same release PR is updated again and may now become `0.2.0`.
6. When the release PR is merged, `0.2.0` is actually released.

So a merged feature or fix does not immediately create a new released version. It only changes what the next release PR contains.

## When a release PR is created

`release-plz` checks every new commit merged into `main`.

That means any merged change can update the pending release PR, including documentation and chore-only changes.

`release-plz` only opens or updates the release PR when the changed files belong to the packaged project. In practice, for this repository, that means changes to shipped project files count, while unrelated repository-only files may not.

Version bump rules are described in the "How version bumps work" section above.

## GitHub App Setup

Release automation uses a GitHub App to create releases that can trigger downstream workflows.

### One-time setup

1. Run the setup script to create and configure the GitHub App:

   ```bash
   ./.github/setup-github-app.sh
   ```

2. Follow the prompts to:
   - Create the GitHub App with appropriate permissions
   - Install it on this repository and on `andrey-fomin/homebrew-tap`
   - Configure repository secrets and variables

3. After setup, verify the configuration:

   ```bash
   gh variable list
   gh secret list
   ```

   You should see `APP_ID` as a variable and `APP_PRIVATE_KEY` as a secret.

### App permissions

The GitHub App requires:
- **Contents**: Read & write (for creating releases, tags, and Homebrew tap update branches)
- **Pull requests**: Read & write (for creating release PRs and Homebrew tap update PRs)

### How it works

1. The `Release PR` workflow uses `actions/create-github-app-token` to generate a token
2. This token is passed to `release-plz/action` which creates/updates release PRs and releases
3. Because the token comes from a GitHub App (not `GITHUB_TOKEN`), the resulting release event triggers the `Release` workflow
4. The `Release` workflow builds and publishes artifacts and Docker images
5. The same `Release` workflow opens or updates a PR in `andrey-fomin/homebrew-tap`

## Notes

- release PRs are created automatically and should not be edited by hand unless needed
- Homebrew tap update PRs are created automatically after each published release
- crates.io publishing is intentionally disabled for now
