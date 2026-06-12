# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-06-13

### Fixed
- Silently ignore bare `-` and `--` arguments instead of erroring

### CI
- Fix workflow permissions for code scanning (GHSA alert)

### Added
- `--explain` flag to suggest install commands
- Status mode showing ✔/✘ for each formula
- Color output support
- Test suite

## [0.6.0] - 2026-06-11

### Changed
- Align lookup CLI behaviour with official `brew which-formula`

## [0.5.0] - 2026-06-11

### Changed
- Improve update flag handling (`--update`)

## [0.4.1] - 2026-06-11

### Chore
- Release bump

## [0.4.0] - 2026-06-11

### Added
- CI skip: suppress lookup inside a pipe or Midnight Commander (`MC_SID`)

## [0.3.1] - 2026-06-11

### Added
- Linux (Linuxbrew) support and Linux release builds

### Docs
- Add `brew trust` step to install instructions

## [0.2.0] - 2026-06-11

### Added
- Release workflow
- `--init` flag for standalone shell integration

### Docs
- Update README description and simplify shell integration wording

### Chore
- Add LICENSE, bump version to 0.2.0

## [0.1.0] - 2026-06-11

### Added
- Initial release: fast drop-in for `brew which-formula --explain`
- Whole-word matching against Homebrew's `executables.txt`
- macOS and Linux cache path resolution without subprocess
- `--update` flag to trigger `brew update --auto-update`
- `--no-warn` flag / `HOMEBREW_NO_CNF_WARN=1` to suppress stale-DB warnings
- Exit codes matching Homebrew (0 on match, 1 on no match)

[Unreleased]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.2.0...v0.3.1
[0.2.0]: https://github.com/cytsai1008/brew-cnf-rust/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/cytsai1008/brew-cnf-rust/releases/tag/v0.1.0
