# Conventional Commits Validator

[![CI](https://github.com/andrey-fomin/conventional-commits-validator/actions/workflows/ci.yml/badge.svg)](https://github.com/andrey-fomin/conventional-commits-validator/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/andrey-fomin/conventional-commits-validator)](https://github.com/andrey-fomin/conventional-commits-validator/releases)
[![Docker](https://img.shields.io/badge/docker-andreyfomin%2Fccval-blue)](https://hub.docker.com/r/andreyfomin/ccval)

Validate commit messages using the Conventional Commits format with YAML configuration.

## Installation

### crates.io

```bash
cargo install ccval
```

### GitHub Releases

Download prebuilt binaries from [GitHub Releases](https://github.com/andrey-fomin/conventional-commits-validator/releases) for Linux, macOS, and Windows.

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
docker run --rm -v $(pwd):/repo -w /repo andreyfomin/ccval
```

### GitHub Action

Use as a GitHub Action in your workflows:

```yaml
on: pull_request

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      - uses: andrey-fomin/conventional-commits-validator@v0
```

The action automatically:
- Validates all commits in a PR (uses `--no-merges`)
- Validates the last commit on push events
- Discovers `conventional-commits.yaml` or `.github/conventional-commits.yaml`

**With custom config:**

```yaml
- uses: andrey-fomin/conventional-commits-validator@v0
  with:
    config: '.github/ccval.yaml'
```

**Override git arguments:**

```yaml
- uses: andrey-fomin/conventional-commits-validator@v0
  with:
    git-args: 'origin/main..HEAD --no-merges'
```

## Usage

```
Usage: ccval [-c <path>] [-r <path>] [-- <git-log-args>...]
       ccval [-c <path>] --stdin
       ccval [-c <path>] -f <path>
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
  -r, --repository <path>
                       Path to Git repository working tree
                       (incompatible with --stdin and --file)

Examples:
  ccval                              # validate last commit
  ccval -- origin/main..HEAD         # validate commits on branch
  ccval -r /path/to/repo             # validate last commit in specific repo
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

## Examples

```bash
ccval                              # validate last commit
ccval -- origin/main..HEAD         # validate commits on branch
ccval -r /path/to/repo             # validate last commit in specific repo
ccval -r . -c config.yaml          # use custom config
printf 'feat: msg\n' | ccval --stdin
ccval --file .git/COMMIT_EDITMSG
ccval -c config.yaml --stdin
```

## Configuration

Create `conventional-commits.yaml` in your working directory:

```yaml
preset: strict

scope:
  required: true
  values:
    - api
    - core
    - ui

type:
  values:
    - feat
    - fix
    - docs
    - refactor
```

### Presets

- `default` - formatting rules for description spacing
- `strict` - `default` plus header length limits and common type restrictions

### Available Fields

```yaml
preset: default

message: <RULES>
header: <RULES>
type: <RULES>
scope: <RULES>
description: <RULES>
body: <RULES>
footer-token: <RULES>
footer-value: <RULES>
footers:
  Closes: <RULES>
```

### Validation Rules

| Rule | Description |
|------|-------------|
| `max-length` | Maximum total length (including newlines) |
| `max-line-length` | Maximum line length (excluding newlines) |
| `required` | Field must be present |
| `forbidden` | Field must not be present |
| `regexes` | List of regexes (all must match) |
| `values` | List of allowed values |

## Building from Source

```bash
cargo build --release
```

The binary will be at `./target/release/ccval`.