# Hen Local Translator Product Roadmap

This file is the source of truth for product-release, subscription, licensing,
and usage-tracking work. Update it whenever work starts, finishes, changes
scope, or becomes blocked.

Last updated: 2026-07-20

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

- ⛔ **Blocked:** Phase 1 release automation is implemented, but the first
  signed/notarized run requires a Developer ID Application certificate and the
  five documented GitHub Actions secrets.

Next item: obtain/export the Developer ID Application certificate and configure
the five required GitHub Actions secrets so signing/notarization can be tested.

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
- [x] Workspace, Tauri, and frontend versions currently agree on `1.1.0`.
- [x] macOS `.app` and DMG build scripts exist.
- [x] A macOS update installation helper exists.
- [x] An update smoke-test script exists.
- [x] Git history contains an older background GitHub updater prototype in
  commit `0b65455`.
- [ ] The current Tauri application has a production updater integration.
- [ ] The repository has an automated GitHub release workflow.
- [ ] Release artifacts are Developer ID signed and Apple notarized.
- [ ] Update artifacts are cryptographically signed and verified by the app.
- [ ] Accounts, billing, subscriptions, and device licensing exist.
- [ ] Local session usage and savings tracking exist.

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
- [ ] Verify the workflow builds the frontend and all required Rust/ML
  executables on GitHub's Apple Silicon runner; implementation is complete and
  awaits the first test tag.
- [ ] Sign embedded executables in the correct order.
- [ ] Sign the app with an Apple Developer ID Application certificate.
- [ ] Enable hardened runtime with the required entitlements.
- [ ] Verify DMG creation on the GitHub runner; implementation is complete and
  awaits the first test tag.
- [ ] Submit the app/DMG to Apple notarization and staple the ticket.
- [ ] Generate Tauri updater artifacts and signatures.
- [ ] Generate `latest.json`; SHA-256 checksum generation is complete.
- [ ] Publish updater artifacts and signatures; DMG, checksums, and generated
  notes will be uploaded to a draft GitHub Release after the workflow is
  verified with its first test tag.
- [ ] Enable immutable releases after the draft release flow is verified.
- [ ] Add a documented manual recovery/re-release procedure.

### Required credentials and external setup

- [ ] Confirm an active Apple Developer Program membership.
- [ ] Create/export the Developer ID signing certificate for CI.
- [ ] Configure Apple notarization credentials.
- [ ] Generate the Tauri updater signing keypair.
- [ ] Store all private credentials only in encrypted CI secrets.

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

- [ ] Add and configure the Tauri 2 updater plugin.
- [ ] Embed only the updater public key in the application.
- [ ] Point the stable channel at the stable `latest.json` endpoint.
- [ ] Add a beta endpoint/channel for invited testers.
- [ ] Check for updates shortly after launch and no more than once per day.
- [ ] Add **Check for Updates** to Settings/About.
- [ ] Display version, release notes, download size, and progress.
- [ ] Add **Restart and Update**, **Install When I Quit**, and **Later**.
- [ ] Prevent an automatic restart during an active translation session.
- [ ] Remove or retire the insecure custom DMG download path after migration.
- [ ] Update the existing smoke test for the Tauri updater format.

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

- [ ] Create development and production backend environments.
- [ ] Implement account creation, sign-in, sign-out, and account recovery.
- [ ] Configure one `$49/month` Stripe price.
- [ ] Implement Stripe Checkout.
- [ ] Implement the Stripe Customer Portal.
- [ ] Verify webhook signatures and process events idempotently.
- [ ] Store only the billing fields required for access decisions.
- [ ] Support `trialing`, `active`, `past_due`, `canceled`, and `unpaid`.
- [ ] Preserve access through `current_period_end` after cancellation.
- [ ] Define and implement the payment-failure grace period.
- [ ] Add a daily billing reconciliation job for missed webhook recovery.
- [ ] Add structured security/audit logs without transcript or audio content.

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

- [ ] Implement browser-based desktop authentication and secure callback.
- [ ] Generate and store a per-device private key in macOS Keychain.
- [ ] Register device public keys and friendly names with the backend.
- [ ] Enforce a maximum of two active devices in a database transaction.
- [ ] Issue signed, device-bound seven-day license leases.
- [ ] Verify leases locally using an embedded public key.
- [ ] Refresh leases daily when the network is available.
- [ ] Add subscription status to the desktop account/settings page.
- [ ] Add a two-device management screen with last-used timestamps.
- [ ] Let the customer choose which device to deactivate.
- [ ] Block a third activation with a clear recovery path.
- [ ] Keep settings and existing transcript export accessible after expiry.
- [ ] Disable new translation only after trial/subscription/grace expiry.

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

- [ ] Define a translation session as time when the translation pipeline is
  running.
- [ ] Pause the timer when translation is paused or stopped.
- [ ] Track current-session, monthly, and lifetime translation duration.
- [ ] Track the number of completed sessions.
- [ ] Persist progress every 30 seconds and at clean shutdown/session end.
- [ ] Recover sensibly after an app crash.
- [ ] Add a session timer to the live translation UI.
- [ ] Add a usage/value section to Settings.
- [ ] Add a transparent, configurable cloud-equivalent comparison rate.
- [ ] Label calculated value as an estimate, not guaranteed savings.
- [ ] State clearly that audio remains local and usage is not used for billing.
- [ ] Keep all detailed usage local by default.

### Acceptance criteria

- [ ] Timer totals survive restart and crashes within the persistence window.
- [ ] Paused or stopped translation is not counted.
- [ ] The displayed estimate exposes its rate and formula.
- [ ] Subscription access is never affected by usage totals.

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
  bundled assets.
- [ ] Document the implications of the existing Apache-2.0 source license.
- [ ] Publish privacy, terms, cancellation, and refund policies.
- [ ] Confirm that no audio or transcript content reaches licensing services.

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

## Progress log

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
