# Conventional Commits Validator

[![CI](https://github.com/andrey-fomin/ccval/actions/workflows/ci.yml/badge.svg)](https://github.com/andrey-fomin/ccval/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/andrey-fomin/ccval)](https://github.com/andrey-fomin/ccval/releases)
[![Docker](https://img.shields.io/badge/docker-andreyfomin%2Fccval-blue)](https://hub.docker.com/r/andreyfomin/ccval)

Validate commit messages using the Conventional Commits format with YAML configuration.

## Installation

### crates.io

```bash
cargo install ccval
```

### GitHub Releases

Download prebuilt binaries from [GitHub Releases](https://github.com/andrey-fomin/ccval/releases) for Linux, macOS, and Windows.

### macOS

On macOS, you may see a warning: "Apple could not verify 'ccval' is free of malware."

To bypass Gatekeeper, run:

```bash
xattr -d com.apple.quarantine /path/to/ccval
```

Alternatively, right-click the binary > Open > Open when prompted.

### Docker

Images are available on Docker Hub: `andreyfomin/ccval`

| Tag | Base | Git Support | Size |
|-----|------|-------------|------|
| `:latest` | Alpine | Yes | ~11 MB |
| `:distroless` | Distroless | No | ~1 MB |

Use the `:distroless` variant for smaller images when only using stdin or file mode.

**Validate stdin:**

```bash
printf 'feat: new feature\n' | docker run --rm -i andreyfomin/ccval --stdin
```

**Validate git commits (Alpine image only):**

```bash
docker run --rm -v $(pwd):/repo -w /repo andreyfomin/ccval --trust-repo
```

### GitHub Action

Use it in your workflow:

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

The action automatically:
- Validates all commits in a PR (uses `--no-merges`)
- Validates the last commit on push events
- Discovers `conventional-commits.yaml` or `.github/conventional-commits.yaml`
- Supports the `preset` input (`default` or `strict`)

Use `@v0` to track the latest compatible `v0.x.y` release, or pin to a specific release tag like `@v0.3.1`. For a truly immutable reference, pin the action to a commit SHA instead of a tag.

**Custom config:**

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    config: '.github/ccval.yaml'
```

**Built-in preset:**

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    preset: strict
```

**Override git arguments:**

```yaml
- uses: andrey-fomin/ccval@v0
  with:
    git-args: 'origin/main..HEAD --no-merges'
```

## Usage

```
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

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Validation failed |
| 2 | Parse error |
| 3 | Config error |
| 4 | CLI usage error |
| 5 | I/O error |
| 6 | Git error |

## How It Works

`ccval` works in two steps:

1. It parses the commit message structure.
2. It applies validation rules from your config.

To avoid ambiguity in this document:

- a message is **parseable** if its structure can be parsed
- a message **passes validation** if the parsed fields satisfy the configured rules

A commit message can be parseable and still fail validation.

See [`PARSING.md`](PARSING.md) for commit message grammar and parse errors.

See [`VALIDATION.md`](VALIDATION.md) for available fields, rule types, presets, and configuration examples.

## Configuration

Configuration is defined in `conventional-commits.yaml` in your working directory.

### Presets

- `default` - formatting rules for description spacing and newline handling in body/footer values
- `strict` - `default` plus header length limits and common type/scope restrictions

Use `-p/--preset` to select a built-in preset from the command line without changing your config file.

Example:

```yaml
preset: strict

type:
  values:
    - feat
    - fix
    - docs

scope:
  required: true
  values:
    - api
    - core
    - ui

header:
  max-line-length: 50
```

## Building from Source

```bash
cargo build --release
```

The binary will be at `./target/release/ccval`.
