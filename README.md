# Conventional Commits Validator

[![CI](https://github.com/andrey-fomin/ccval/actions/workflows/ci.yml/badge.svg)](https://github.com/andrey-fomin/ccval/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/andrey-fomin/ccval)](https://github.com/andrey-fomin/ccval/releases)
[![Docker](https://img.shields.io/badge/docker-andreyfomin%2Fccval-blue)](https://hub.docker.com/r/andreyfomin/ccval)

Validate commit messages locally, in CI, or from stdin using Conventional Commits rules.

`ccval` helps you:
- catch invalid commit messages before merge or release
- enforce team rules for types, scopes, headers, and footers
- validate one commit, a branch range, or a message from stdin or a file

## Quickstart

Once `ccval` is installed, validate the last commit in the current repository:

```bash
ccval
```

Validate all commits on your branch:

```bash
ccval -- origin/main..HEAD
```

Use the built-in strict preset:

```bash
ccval -p strict
```

Validate a commit message from stdin:

```bash
printf 'feat: add validation\n' | ccval --stdin
```

## Install

Choose the option that fits your workflow.

### crates.io

```bash
cargo install ccval
```

### Homebrew

```bash
brew install andrey-fomin/tap/ccval
```

### GitHub Releases

Download prebuilt binaries for Linux, macOS, and Windows from [GitHub Releases](https://github.com/andrey-fomin/ccval/releases).

### Docker

Run `ccval` without installing it locally:

```bash
printf 'feat: add validation\n' | docker run --rm -i andreyfomin/ccval:distroless --stdin
```

Available image tags (moving convenience tags):

- `andreyfomin/ccval:latest` - Alpine-based, includes Git support, about 11 MB
- `andreyfomin/ccval:distroless` - minimal image, no Git support, about 1 MB

- Use `:distroless` when you only need stdin or file validation
- Use `:latest` when you need to validate commits from a Git repository

The release workflow also publishes versioned tags, including `:1`, `:1.2`, `:1.2.3`, and matching `-distroless` variants such as `:1.2.3-distroless`.

For Git-based validation in a mounted repository, use `--trust-repo` only when you control the repository and Git fails with a `detected dubious ownership` warning:

```bash
docker run --rm -v "$(pwd)":/repo -w /repo andreyfomin/ccval --trust-repo
```

### macOS note

If macOS blocks a downloaded binary, remove the quarantine attribute:

```bash
xattr -d com.apple.quarantine /path/to/ccval
```

You can also right-click the binary, choose Open, and confirm the prompt.

## Common Tasks

### Validate the latest commit

```bash
ccval
```

### Validate a commit range

```bash
ccval -- origin/main..HEAD
```

### Validate commits in another repository

```bash
ccval -r /path/to/repo -- HEAD~10..HEAD
```

`--repository` changes where Git reads commits from. Config auto-discovery still happens in the current working directory, so use `--config` too when the target repository has its own config file:

```bash
ccval -r /path/to/repo -c /path/to/repo/conventional-commits.yaml -- HEAD~10..HEAD
```

### Validate a message file

```bash
ccval --file .git/COMMIT_EDITMSG
```

### Validate in a container or ownership-mismatch environment

```bash
ccval -T
```

Or with an explicit repository path:

```bash
ccval -r /repo -T -- HEAD~5..HEAD
```

## Git Hook

Use a `commit-msg` hook to validate each commit message before Git creates the commit.

Create `.git/hooks/commit-msg` with this content:

```sh
#!/bin/sh

set -eu

exec ccval --file "$1"
```

Then make it executable:

```bash
chmod +x .git/hooks/commit-msg
```

This hook expects `ccval` to be installed and available on your `PATH`.

## Configuration in 30 Seconds

By default, if no config file is found and no preset is specified, `ccval` only checks whether the commit message is parseable as a Conventional Commit.

When a config file is present or a preset is provided, `ccval` also applies the validation rules defined there so you can enforce team-specific rules.

To enforce team-specific rules, add a config file such as `conventional-commits.yaml`.

Minimal example:

```yaml
preset: strict

type:
  values:
    - feat
    - fix
    - docs

scope:
  required: true
```

The same idea in TOML:

```toml
preset = "strict"

[type]
values = ["feat", "fix", "docs"]

[scope]
required = true
```

With this config:

- `strict` enables useful default formatting rules
- only `feat`, `fix`, and `docs` are allowed
- every commit must include a scope such as `feat(api): add endpoint`

When `--config` is not provided, `ccval` looks for these files in order:

- `conventional-commits.yaml`
- `conventional-commits.yml`
- `conventional-commits.toml`
- `conventional-commits.json`

Use a custom config path when needed:

```bash
ccval -c .github/conventional-commits.yaml -- origin/main..HEAD
```

## Presets

`ccval` includes two built-in presets:

- `default` - formatting rules for description spacing and newline handling in body and footer values
- `strict` - `default` plus header length limits and common type and scope restrictions

When a config file and preset are both used:

- `-p/--preset` takes precedence over `preset:` in the config file
- rules omitted in your config inherit from the preset
- `regexes: []` clears preset regex rules for that field

Use a preset from the command line without changing your config file:

```bash
ccval -p strict
```

## GitHub Action

Use `ccval` in GitHub Actions to validate pull request commit ranges or pushed commits.

```yaml
on: pull_request

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      - uses: andrey-fomin/ccval@v0
```

Common options:

Use a custom config:

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    config: .github/conventional-commits.yaml
```

Use a built-in preset:

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    preset: strict
```

Override git arguments:

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    git-args: origin/main..HEAD --no-merges
```

Note: `git-args` is parsed as a whitespace-separated list of arguments by the action; shell-style quoting/escaping is not supported, and arguments that contain spaces cannot be passed as a single argument. Auto-detected behavior such as adding `--no-merges` applies only when `git-args` is not set.

Limit checked commits:

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    max-commits: "250"
```

The action supports `push` and `pull_request` events, discovers `conventional-commits.yaml` or `.github/conventional-commits.yaml`, skips merge commits in its auto-detected ranges, skips deleted-ref and zero-commit pushes, and limits validation to 100 commits by default.

For push events, it prefers the exact pushed range from local history. If the pushed `before` commit is not available locally, it falls back to the default-branch merge-base when possible and otherwise fails with a clear error.

If either the push event itself or the selected commit range exceeds `max-commits`, the action skips validation, emits a warning, and exits successfully.

Make sure your workflow fetches enough history, for example with `fetch-depth: 0`, so the required commit range is available.

Use `@v0` to track the latest compatible `v0.x.y` release, or pin a specific release tag such as `@v0.3.1`. For a fully immutable reference, pin a commit SHA.

## Parsing vs Validation

`ccval` checks commit messages in two steps:

1. Parse the message structure
2. Apply validation rules from your config

A message can be parseable and still fail validation.

For example, `feat:  add api ` may parse successfully but fail stricter formatting rules.

Read more:

- [`PARSING.md`](PARSING.md) for commit message structure and parse errors
- [`VALIDATION.md`](VALIDATION.md) for available fields, rules, and configuration examples

## Command Reference

```text
Usage: ccval [-c <path>] [-p <preset>] [-r <path>] [-T] [-- <git-log-args>...]
       ccval [-c <path>] [-p <preset>] --stdin
       ccval [-c <path>] [-p <preset>] -f <path>
       ccval -h

Validates commit messages from stdin, a file, or Git.

Modes:
  (default)            Validate commit(s) from git log
                       Use -- <git-log-args>... to pass arguments to git log
                       Default: -1 (last commit)

  --stdin              Read commit message from stdin
  -f, --file <path>    Read commit message from a file
  -h, --help           Show this help message

Options:
  -c, --config <path>  Use a custom config file path
  -p, --preset <name>  Use a built-in preset (default or strict)
  -r, --repository <path>
                       Path to Git repository working tree
                       Cannot be used with --stdin or --file
  -T, --trust-repo     Trust the repository despite ownership mismatch
                       Useful when running in containers or accessing
                       repositories owned by other users
                       Requires git mode (cannot use with --stdin or --file)

Examples:
  ccval                              # validate last commit
  ccval -- origin/main..HEAD         # validate commits on branch
  ccval -p strict                    # validate last commit with strict preset
  ccval -r /path/to/repo             # validate last commit in specific repo
  ccval -T                           # validate last commit, trusting repo
  ccval -r /repo -T                  # validate in container
  printf 'feat: msg\n' | ccval --stdin
  ccval --file .git/COMMIT_EDITMSG
  ccval -c config.yaml --stdin
```

## Exit Codes

- `0` success
- `64` usage error
- `65` content invalid
- `66` input unavailable
- `70` internal error
- `74` I/O error
- `77` permission error
- `78` config error

## Reference

- [`PARSING.md`](PARSING.md) explains the supported commit message structure
- [`VALIDATION.md`](VALIDATION.md) lists fields, rule types, presets, and examples
- [`CHANGELOG.md`](CHANGELOG.md) tracks release history
- [`RELEASING.md`](RELEASING.md) documents the release process
