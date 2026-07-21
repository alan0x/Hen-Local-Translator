# Hen Local account and billing backend

This directory contains the deployable Supabase/Stripe foundation for the one
Hen Local plan: 7-day trial, $49/month, unlimited local translation, two active
Macs, and seven days of offline license access.

No audio, transcript, or detailed local usage data belongs in this backend.

## Environments

Create separate Supabase projects and Stripe test/live configurations for
development and production. Copy `.env.example` to an ignored local env file;
never commit real secrets.

Required external setup:

1. Create a Stripe recurring monthly price for exactly $49 USD and set
   `STRIPE_MONTHLY_PRICE_ID`.
2. Configure the Stripe webhook endpoint for `stripe-webhook`.
3. Generate an Ed25519 PKCS#8 license signing key with
   `scripts/generate_license_key.sh`. Keep the generated private Base64 file in
   the protected `LICENSE_PRIVATE_KEY_PKCS8_BASE64` deployment secret; the
   desktop app contains only `license-public-key.b64`.
4. Apply migrations, deploy functions, and configure secrets with the Supabase
   CLI or dashboard.
5. Schedule `billing-reconcile` daily using a secret header.

The protected GitHub workflow **Deploy account and licensing backend** applies
the migration, configures secrets, and deploys all four functions. The separate
**Daily billing reconciliation** workflow calls the recovery function once per
day. Create `development` and `production` GitHub Environments with the secrets
and variables named in those workflow files before running them.

## Desktop authentication configuration

The release build accepts these public compile-time values:

- `HEN_LOCAL_SUPABASE_URL`
- `HEN_LOCAL_SUPABASE_ANON_KEY`
- `HEN_LOCAL_AUTH_PROVIDER` (defaults to `google`)
- `HEN_LOCAL_ACCOUNT_URL` (optional branded browser sign-in page)

The release workflow reads them from protected GitHub secrets/variables. For a
local internal test, copy
`hen-local-translator-shell/account_config.example.json` to
`~/Library/Application Support/Hen Local Translator/account_config.json`.

In Supabase Auth, enable the chosen provider and allow the redirect pattern
`henlocal://auth/callback*`. The desktop app uses PKCE and validates its own
random callback state. The PKCE verifier, refreshed session, and per-device
Ed25519 private key are stored in macOS Keychain. The callback scheme is
registered in the signed macOS bundle and should be tested from the installed
Applications copy, not only `cargo run`.

The optional branded account page must preserve the supplied `redirect_to`,
`code_challenge`, `code_challenge_method`, and `state` values when it starts the
Supabase authorization request. It must never receive or store the desktop
device private key.

The repository does not currently contain live Supabase/Stripe credentials, so
these files are implementation-ready but not deployed.
