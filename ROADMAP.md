# Hen Local Translator Product Roadmap

This file is the source of truth for product-release, subscription, licensing,
and usage-tracking work. Update it whenever work starts, finishes, changes
scope, or becomes blocked.

Last updated: 2026-07-21

## How to use this tracker

- Keep exactly one item under **Current focus** marked as in progress.
- Check an item only after its acceptance criteria are verified.
- Add newly discovered work to the appropriate phase instead of keeping a
  separate private to-do list.
- Record important scope or product decisions under **Decisions**.
- Add a dated entry to **Progress log** whenever a milestone changes state.
- Do not place secrets, signing keys, customer data, or credentials here.

Status notation:

- `[ ]` Not started
- `[x]` Completed and verified
- `🔄` In progress
- `⛔` Blocked; explain the blocker beside the item

## Current focus

- 🔄 **In progress:** Deploy and connect the account/licensing foundation once
  Supabase, Stripe, login-provider, Apple signing, and voice-asset credentials
  are supplied; meanwhile complete all locally testable release work.

Apple signing/notarization remains a production-release blocker. Once the paid
Developer Program membership is active, configure the five required GitHub
Actions secrets and rerun the release flow with a new prerelease version.

## Product definition

Hen Local has one paid product, not multiple feature tiers.

| Item | Rule |
| --- | --- |
| Trial | 7 days, beginning with first app activation |
| Subscription | $49 USD per month |
| Translation | Unlimited local translation |
| Devices | Up to 2 active computers per account |
| Offline operation | Up to 7 days after the most recent successful license refresh |
| Cancellation | Access continues through the paid-through date |
| Usage minutes | Displayed for customer value; never used for billing |

Not included at launch:

- No Solo tier.
- No event pass.
- No minute bundles or overage charges.
- No annual billing until the monthly product has been validated.

## Existing foundation

- [x] Rust/Tauri 2/Svelte desktop application exists.
- [x] Workspace, Tauri, and frontend versions currently agree on
  `1.2.0-beta.1`.
- [x] macOS `.app` and DMG build scripts exist.
- [x] A macOS update installation helper exists.
- [x] An update smoke-test script exists.
- [x] Git history contains an older background GitHub updater prototype in
  commit `0b65455`.
- [x] The current Tauri application has a production updater integration.
- [x] The repository has an automated GitHub release workflow.
- [ ] Release artifacts are Developer ID signed and Apple notarized.
- [x] Update artifacts are cryptographically signed and verified by the app.
- [x] Accounts, billing, subscriptions, and device licensing are implemented;
  production deployment and live-service acceptance testing remain external.
- [x] Local session usage and value-estimate tracking exist.

## Phase 1: Release version control and macOS distribution

Goal: one Git tag produces a reproducible, trusted macOS release.

### Version policy

- Patch releases such as `1.1.1` contain compatible bug fixes.
- Minor releases such as `1.2.0` contain compatible new features.
- Major releases such as `2.0.0` contain breaking changes.
- Prereleases use tags such as `1.2.0-beta.1`.
- Published release tags must never be moved or reused.
- `main` should remain releasable.

### Tasks

- [x] Treat the workspace version in `Cargo.toml` as the source of truth.
- [x] Add a command that updates or validates all version locations:
  - workspace `Cargo.toml`
  - `hen-local-translator-shell/tauri.conf.json`
  - `hen-local-translator-shell/ui/package.json`
  - `hen-local-translator-shell/ui/package-lock.json`
  - generated macOS bundle metadata
- [x] Fail CI when version locations disagree.
- [x] Add a changelog and release-note convention.
- [x] Add a GitHub Actions workflow triggered by `v*` tags.
- [x] Verify the workflow builds the frontend and all required Rust/ML
  executables on GitHub's Apple Silicon runner.
- [ ] Sign embedded executables in the correct order.
- [ ] Sign the app with an Apple Developer ID Application certificate.
- [ ] Enable hardened runtime with the required entitlements.
- [x] Verify DMG creation on the GitHub runner.
- [ ] Submit the app/DMG to Apple notarization and staple the ticket.
- [x] Generate Tauri updater artifacts and signatures.
- [x] Generate `latest.json`; SHA-256 checksum generation is complete.
- [ ] Publish updater artifacts and signatures; DMG, checksums, and generated
  notes will be uploaded to a draft GitHub Release after the workflow is
  verified with its first test tag.
- [ ] Enable immutable releases after the draft release flow is verified.
- [x] Add a documented manual recovery/re-release procedure.

### Required credentials and external setup

- [ ] Confirm an active Apple Developer Program membership.
- [ ] Create/export the Developer ID signing certificate for CI.
- [ ] Configure Apple notarization credentials.
- [x] Generate the Tauri updater signing keypair.
- [x] Store the updater private credential in an encrypted CI secret; Apple
  credentials remain pending.
- [x] Permit an explicitly labeled unsigned draft for internal CI testing while
  Apple Developer Program activation is pending.

### Acceptance criteria

- [ ] Tagging `v1.2.0-beta.1` creates a complete draft release without manual
  filename or version edits.
- [ ] A clean supported Mac installs and launches the build without bypassing
  Gatekeeper.
- [ ] `codesign`, `spctl`, and notarization verification all succeed.
- [ ] A release artifact can be traced back to its immutable tag and commit.

## Phase 2: In-application signed updates

Goal: customers can safely discover and install new versions from the app.

### Tasks

- [x] Add and configure the Tauri 2 updater plugin.
- [x] Embed only the updater public key in the application.
- [x] Point the stable channel at the stable `latest.json` endpoint.
- [ ] Add a beta endpoint/channel for invited testers.
- [x] Check for updates shortly after launch and no more than once per day.
- [x] Add **Check for Updates** to Settings/About.
- [x] Display version, release notes, download size, and progress.
- [x] Add **Restart and Update**, **Install When I Quit**, and **Later**.
- [x] Prevent an automatic restart during an active translation session.
- [x] Remove or retire the insecure custom DMG download path after migration.
- [x] Update the existing smoke test for the Tauri updater format.

### Acceptance criteria

- [ ] No update is shown when the installed version is current.
- [ ] A valid newer release downloads, verifies, installs, and relaunches.
- [ ] Tampered or unsigned artifacts are rejected.
- [ ] Interrupted downloads recover cleanly.
- [ ] Updating works from at least the previous two supported versions.
- [ ] An active translation session is never interrupted without consent.

## Phase 3: Account, billing, and entitlement backend

Goal: one subscription record controls access without processing audio in the
cloud.

Initial implementation choice:

- Supabase Auth and Postgres for accounts and product records.
- A small server-side API or Supabase Edge Functions.
- Stripe Billing, Checkout, webhooks, and Customer Portal.

Paddle may be evaluated before public international sales if merchant-of-record
tax handling is preferable. This decision must not change the desktop license
format.

### Minimal data model

- `users`
- `subscriptions`
- `devices`
- `license_leases`
- `webhook_events`

### Tasks

- [ ] ⛔ Create development and production backend environments (requires the
  production Supabase organization/project access).
- [ ] ⛔ Implement end-user account creation, sign-in, sign-out, and recovery
  (server schema exists; login provider and public account URL are required).
- [ ] ⛔ Configure one `$49/month` Stripe price (requires the Stripe account).
- [x] Implement Stripe Checkout.
- [x] Implement the Stripe Customer Portal.
- [x] Verify webhook signatures and process events idempotently.
- [x] Store only the billing fields required for access decisions.
- [x] Support `trialing`, `active`, `past_due`, `canceled`, and `unpaid`.
- [x] Preserve access through `current_period_end` after cancellation.
- [x] Define and implement a fixed three-day payment-failure grace period.
- [x] Add a daily billing reconciliation function for missed webhook recovery.
- [x] Add structured security/audit logs without transcript or audio content.

### Acceptance criteria

- [ ] A customer can start the trial, subscribe, update payment details, and
  cancel without staff intervention.
- [ ] Duplicate or out-of-order webhook deliveries do not corrupt access.
- [ ] Billing-provider downtime does not immediately stop a licensed app.
- [ ] No billing secrets are shipped inside the desktop application.

## Phase 4: Two-device licensing and desktop subscription UX

Goal: one account works on two computers with reasonable offline support.

### License behavior

1. The customer signs in through a secure browser-based flow.
2. The app generates a device keypair.
3. The private key is stored in macOS Keychain.
4. The backend registers the public key and a recognizable device name.
5. The backend issues a signed license lease valid for seven days.
6. The app verifies the lease locally and refreshes it once per day.

### Tasks

- [x] Implement browser-based PKCE desktop authentication and a validated,
  bundle-registered `henlocal://` callback.
- [x] Generate and store a per-device Ed25519 private key in macOS Keychain.
- [x] Register device public keys and friendly names with the backend.
- [x] Enforce a maximum of two active devices in a database transaction.
- [x] Issue signed, device-bound seven-day license leases on the backend.
- [x] Verify canonical lease payloads locally using an embedded Ed25519 public
  key, device binding, issue/expiry checks, and a maximum seven-day window.
- [x] Refresh leases on launch and once daily when the network is available.
- [x] Add subscription, paid-through, and offline-license status to Settings.
- [x] Add a two-device management screen with last-used timestamps.
- [x] Let the customer choose which device to deactivate; deactivating the
  current Mac also signs it out instead of silently reactivating it.
- [x] Block a third activation with the active-device list and a clear path to
  deactivate one before retrying.
- [x] Keep settings and existing transcript export accessible after expiry.
- [x] Disable only new translation after trial/subscription/grace/lease expiry.

### Acceptance criteria

- [ ] One account activates on two Macs.
- [ ] A third Mac is blocked until one device is deactivated.
- [ ] A valid device continues translating offline for up to seven days.
- [ ] Deactivation frees a slot immediately when the network is available.
- [ ] Cancellation does not revoke access before the paid-through date.
- [ ] Reinstall, Keychain loss, device replacement, and clock changes have
  documented recovery behavior.

## Phase 5: Local usage and value counter

Goal: show product value without metering or uploading customer content.

### Tasks

- [x] Define a translation session as time when the translation pipeline is
  running.
- [x] Pause the timer when translation is stopped or fails.
- [x] Track current-session, monthly, and lifetime translation duration.
- [x] Track the number of completed sessions.
- [x] Persist progress every 30 seconds and at clean shutdown/session end.
- [x] Recover sensibly after an app crash with at most the uncheckpointed window
  omitted rather than inventing elapsed time.
- [x] Add a session timer to the live translation UI.
- [x] Add a usage/value section to Settings.
- [x] Add a transparent, configurable cloud-equivalent comparison rate.
- [x] Label calculated value as an estimate, not guaranteed savings.
- [x] State clearly that audio remains local and usage is not used for billing.
- [x] Keep all detailed usage local by default.

### Acceptance criteria

- [x] Timer totals survive restart and crashes within the 30-second persistence
  window without inventing time after a crash.
- [x] Stopped translation is not counted.
- [x] The displayed estimate exposes its configurable rate and formula.
- [x] Subscription access is independent of usage totals.

## Phase 6: Product, security, and lifecycle testing

Goal: verify full customer journeys and failure recovery before charging users.

- [ ] Install, start trial, subscribe, and receive an entitlement.
- [ ] Let a trial expire without subscribing.
- [ ] Activate two devices and reject a third.
- [ ] Replace or deactivate an old device.
- [ ] Operate offline through the lease window.
- [ ] Cancel while retaining access through the paid-through date.
- [ ] Handle a failed renewal and successful payment recovery.
- [ ] Recover from missing/corrupt local license and Keychain state.
- [ ] Recover from duplicated, delayed, and missed billing webhooks.
- [ ] Update from older supported app versions.
- [ ] Recover from interrupted or corrupted update downloads.
- [ ] Complete a commercial license audit for models, voices, libraries, and
  bundled assets. Model licenses are verified; ownership/consent records for the
  eight bundled preview recordings are still a release blocker.
- [x] Document the implications of the existing Apache-2.0 source license.
- [ ] Publish privacy, terms, cancellation, and refund policies.
- [x] Confirm in the backend schema/API design that no audio, transcript, or
  detailed local usage content reaches licensing services.

## Phase 7: Closed paid beta and production launch

Goal: validate the single subscription before introducing more pricing.

### Release sequence

- [ ] `v1.2.0-beta.1`: signed release pipeline and updater.
- [ ] `v1.2.0-beta.2`: accounts and billing.
- [ ] `v1.2.0-beta.3`: device licensing, offline behavior, and usage counter.
- [ ] `v1.2.0`: first stable paid release, subject to beta results.

### Beta work

- [ ] Recruit approximately 20–50 beta customers.
- [ ] Provide a documented support and license-recovery process.
- [ ] Measure trial-to-paid conversion.
- [ ] Measure update, sign-in, activation, and payment failure rates.
- [ ] Measure support requests per active customer.
- [ ] Measure device replacement frequency and offline-license failures.
- [ ] Collect feedback on the `$49/month` price and two-device allowance.
- [ ] Decide whether to add annual billing only after reviewing beta results.
- [ ] Do not add additional feature tiers during the initial beta.

## Decisions

- 2026-07-20: Use one paid plan: `$49/month`, unlimited local translation,
  and two active computers.
- 2026-07-20: Use a seven-day trial and seven-day offline license lease.
- 2026-07-20: Remove the proposed Solo tier and event pass from launch scope.
- 2026-07-20: Usage minutes communicate value and never determine billing.
- 2026-07-20: Use GitHub Releases for release history and update artifacts,
  with Tauri signatures plus Apple signing/notarization.
- 2026-07-20: Complete secure releases and updates before enforcing paid
  subscriptions.
- 2026-07-21: Keep the application and core translation models separate. The
  app downloads about 4.2 GB of ASR/translation models after first launch;
  spoken translation uses built-in macOS voices and needs no separate model.
- 2026-07-21: Begin the seven-day trial on first device activation, not account
  creation, and use a fixed three-day payment-failure grace period.
- 2026-07-21: Store desktop sessions, PKCE verifier state, and per-device
  private keys in macOS Keychain; ship only the license verification public key.
## Progress log

- 2026-07-21: Added a separate Apple Voice Lab window that reads every system
  voice currently available on the Mac, filters by language, previews a
  consistent long-form sample, and saves/copies a Chinese and English favorite
  shortlist. Fresh installations keep spoken translation off until enabled.
- 2026-07-21: Replaced the optional Qwen TTS dataflow node, audio-player bridge,
  3.1 GB model download, packaged executable, and prerecorded previews with
  five local Apple system-voice choices. Spoken sentences are queued in order,
  stop immediately with translation, follow the Mac's default output, and fall
  back to an installed voice for the target language. A clean bundle contains
  no Qwen TTS executable or preview assets; live translation starts with only
  ASR and translation nodes.
- 2026-07-21: Fixed local translation getting stuck at startup. Bridge retries
  now keep successful connections and reset only failed workers; shutdown
  always stops the Dora flow even if a bridge disconnect reports an error.
  Hen Local flows now use a stable name, single-instance launch is enforced,
  per-process rendered dataflows cannot be overwritten by tests, and both old
  random-name and new crash-orphaned Hen Local flows are safely detected and
  stopped without touching unrelated Dora work. Verified real start, stop,
  second start, forced-crash recovery, and legacy-flow recovery with ASR,
  translation, and Qwen TTS all enabled.
- 2026-07-21: Rebuilt the current 76 MB internal DMG and 65 MB updater archive
  with the official Tauri shell. The app, disk image, updater signature,
  `latest.json`, `henlocal://` installed-app callback, version consistency, and
  process launch smoke tests pass. The bundle is about 204 MB and contains no
  model weights; final two-window visual confirmation remains on the morning
  test checklist.
- 2026-07-21: Added a Keychain-backed monotonic clock watermark and one-plan
  validation to prevent a local clock rollback or an unknown signed plan from
  extending desktop access.
- 2026-07-21: Completed the configurable desktop side of accounts and two-Mac
  licensing: secure browser PKCE callback, Keychain session/device identity,
  server registration, signed local lease validation, daily refresh, paid and
  offline status, device list/deactivation, third-device recovery, and new-
  translation gating. Added protected backend deployment and daily billing-
  reconciliation workflows. Real end-to-end acceptance remains blocked on the
  external Supabase, Stripe, login-provider, and Apple configurations.
- 2026-07-21: Added the Supabase/Stripe account foundation: minimal product
  records, activation-based trial, Checkout, Customer Portal, verified and
  idempotent webhooks, cancellation/paid-through handling, fixed payment grace,
  daily reconciliation, transactional two-device enforcement, audit events,
  and signed seven-day leases. Edge Functions pass Deno type-checking and five
  entitlement tests; deployment is blocked on external service accounts.
- 2026-07-21: Added local current-session, monthly, lifetime, and completed-
  session statistics with 30-second crash-safe checkpoints, a live timer,
  configurable comparison rate, and clearly labeled local-only value estimate.
- 2026-07-21: Added a separate Apple Voice Lab that lists and previews every
  system voice installed on the current Mac. Mapped English choices 1–5 to the
  downloaded Siri Live Speech Voice 1–5; Chinese remains on installed Chinese
  voices until the product shortlist is selected.
- 2026-07-21: Promoted the installed `Yue (Premium)` voice to the first Chinese
  speech candidate after confirming it is available through Apple's public
  speech interface and has low runtime memory overhead. Downloaded Chinese Siri
  Live Speech voices are absent from the current public speech APIs and are not
  wired through private Apple interfaces.
- 2026-07-21: Added launch drafts for privacy, terms, cancellation/refunds,
  support/license recovery, and release recovery. Verified all three downloaded
  MLX model cards declare Apache-2.0; preview-recording provenance remains a
  commercial-release blocker.
- 2026-07-21: Replaced the hand-assembled macOS app wrapper that produced blank
  WebViews in the internal DMG with Tauri's official application bundler. The
  rebuilt app, DMG, signed updater archive, embedded local executables, numeric
  Apple bundle version, and static updater manifest pass local structural and
  signature smoke tests. A locked Mac prevented the final visual launch check.
- 2026-07-21: Added retry-safe first-run core model delivery (about 4.2 GB).
  Models are intentionally excluded from the DMG.
- 2026-07-21: Added the official Tauri updater, stable GitHub endpoint, daily
  automatic checks, manual Settings control, progress, deferred/restart install
  choices, active-session protection, signing key, CI secret, and `latest.json`.
- 2026-07-20: Verified the `v1.2.0-beta.1` unsigned internal-test pipeline on a
  clean GitHub Apple Silicon runner. The generated 91 MB DMG passed its SHA-256
  checksum and disk-image verification, mounted successfully, contained the
  native ARM64 `1.2.0-beta.1` executable and required resources, and passed a
  controlled local launch test. The GitHub Release remains a draft and is
  clearly labeled as unsigned and unsuitable for customers.
- 2026-07-20: Allowed the release workflow to create a clearly labeled unsigned
  internal-test draft when Apple credentials are unavailable; public release
  remains blocked until Developer ID signing and notarization succeed.
- 2026-07-20: Added hardened-runtime signing for nested Mach-O files and the app,
  plus Developer ID DMG signing, Apple notarization, stapling, and Gatekeeper
  assessment. End-to-end execution is blocked until the Developer ID identity
  and Apple/GitHub secrets are configured.
- 2026-07-20: Added a tag-driven Apple Silicon release workflow that validates
  the tag, uses locked dependencies, builds and verifies the app/DMG, generates
  checksums, uploads a workflow artifact, and creates an explicitly unsigned
  draft GitHub Release. Public release remains blocked on signing/notarization.
- 2026-07-20: Added `scripts/version.mjs` with `check`, `sync`, and `set`
  commands; added pull-request/main CI validation; and made macOS packaging
  reject versions that differ from the workspace source of truth.
- 2026-07-20: Created this roadmap after confirming that the current checkout
  did not contain an existing general product roadmap or to-do Markdown file.
- 2026-07-20: Set Phase 1, automated trusted macOS releases, as the current
  focus.
