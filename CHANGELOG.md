# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/andrey-fomin/ccval/compare/v0.2.0...v0.3.0) - 2026-03-20

### Added

- add ccval GitHub Action
- add --trust-repo/-T flag for container usage

### Changed

- update repository references to ccval

## [0.2.0](https://github.com/andrey-fomin/conventional-commits-validator/compare/v0.1.6...v0.2.0) - 2026-03-14

### Added

- add explicit stdin mode, default to git
- add -r/--repository flag

### Changed

- group build matrix entries by platform
- add commit validation self-check
- add test for multiple validation errors
- enforce single-commit PRs
- add Docker Hub metadata update job
- skip validate step for bot-authored PRs

### Fixed

- add workflow_dispatch to enable manual release triggers
- correct artifact upload path after download to current directory

## [0.1.6](https://github.com/andrey-fomin/conventional-commits-validator/compare/v0.1.5...v0.1.6) - 2026-03-14

### Changed

- replace macOS runners with cargo-zigbuild for cross-compilation
- rework release automation around release PRs

### Documentation

- update Docker image sizes in README

## [0.1.5] - 2026-03-14

### Changed
- Reduced the release binary size by replacing `regex` with `regex-lite`
- Tightened the release profile to produce smaller binaries

### Documentation
- Filled in the changelog history for earlier releases

## [0.1.4] - 2026-03-14

### Fixed
- Strip extra newline from Git `%B` output

## [0.1.3] - 2026-03-14

### Added
- Docker images for Alpine and Distroless variants

### Changed
- Updated GitHub Actions to Node.js 24 compatible versions

## [0.1.2] - 2026-03-14

### Added
- ARM64 build targets for Linux and Windows releases

### Changed
- Use standard `LICENSE` file and add license to `Cargo.toml`

## [0.1.1] - 2026-03-14

### Fixed
- Use macos-12 runner for Intel builds
- Prevent duplicate release notes in GitHub releases

### Added
- macOS Gatekeeper workaround documentation

## [0.1.0] - 2026-03-14

### Added
- Initial release in Rust
- Conventional Commits message parsing and validation
- YAML configuration with preset support (`default`, `strict`)
- Multiple input modes: stdin, file, git
- Git integration for validating commit history
- Configurable validation rules (max-length, max-line-length, required, forbidden, regexes, values)
- Exit codes for different error types
- Docker images for distribution
