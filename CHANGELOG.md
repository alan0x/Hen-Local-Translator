# Changelog

Notable changes to Hen Local Translator are recorded here. GitHub automatically
generates detailed release notes from merged pull requests; this file keeps a
short, product-facing history.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and releases use [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- A single-source application version command and CI validation.
- A tag-driven workflow that builds an Apple Silicon DMG and creates a draft
  GitHub Release.
- Developer ID hardened-runtime signing and Apple notarization/stapling for
  release DMGs.

## 1.1.0 - 2026-07-20

### Added

- Tauri 2 and Svelte desktop interface for local live translation.
- Microphone and macOS system-audio translation workflows.
- Floating bilingual subtitle overlay.
- macOS application and DMG packaging scripts.
