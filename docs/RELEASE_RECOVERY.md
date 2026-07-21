# Release and Update Recovery Runbook

Use this when a tag build, draft release, DMG, or updater publication fails.

## Rules

- Never move or reuse a published tag.
- Never publish an unsigned or unnotarized build to customers.
- Keep failed internal artifacts in a clearly labeled draft release only.
- Do not edit `latest.json` by hand or point it at an artifact from a different
  commit/version.

## Failed build before publication

1. Leave the failed tag and workflow history intact for traceability.
2. Fix the problem on the release branch.
3. Increment the prerelease number or patch version using `scripts/version.mjs`.
4. Commit, push, and create a new tag.

## Bad draft artifact

Delete only the affected draft release assets, not Git history. Build a new
version/tag and replace the entire draft release set: DMG, updater archive,
signature, checksums, manifest, and notes. Verify all files come from one run.

## Bad public release

1. Remove the affected version from the updater manifest immediately so no new
   automatic installs are offered.
2. Mark the GitHub release as affected; do not overwrite its artifacts.
3. Publish a new fixed version and tag.
4. If the installed app cannot update itself, provide the newly notarized DMG
   and plain-language reinstall steps. Reinstall must preserve preferences,
   transcripts, models, Keychain device identity, and local usage totals.

## Required checks before republishing

- Version files and tag agree.
- Frontend and Rust tests pass.
- Embedded sidecars are present and signed in order.
- App and DMG pass `codesign`, Gatekeeper, notarization, and stapling checks.
- DMG mounts and both app windows render on a clean supported Mac.
- Updater signature matches the archive and a previous supported version can
  download, install, and relaunch without interrupting an active session.
