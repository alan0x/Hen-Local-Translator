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
3. Generate an Ed25519 PKCS#8 license signing key and keep only its public key
   in the desktop app.
4. Apply migrations, deploy functions, and configure secrets with the Supabase
   CLI or dashboard.
5. Schedule `billing-reconcile` daily using a secret header.

The repository does not currently contain live Supabase/Stripe credentials, so
these files are implementation-ready but not deployed.
