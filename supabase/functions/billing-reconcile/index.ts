import { admin, env, stripe } from "../_shared/clients.ts";
import { errorResponse, json } from "../_shared/http.ts";
import { syncSubscription } from "../_shared/stripe-sync.ts";

Deno.serve(async (request) => {
  if (request.method !== "POST") {
    return json({ error: "METHOD_NOT_ALLOWED" }, 405);
  }
  if (
    request.headers.get("x-reconcile-secret") !==
      env("BILLING_RECONCILE_SECRET")
  ) return json({ error: "UNAUTHORIZED" }, 401);
  try {
    const { data, error } = await admin.from("subscriptions").select(
      "stripe_subscription_id",
    ).not("stripe_subscription_id", "is", null);
    if (error) throw error;
    let updated = 0;
    const failures: string[] = [];
    for (const row of data) {
      try {
        const subscription = await stripe.subscriptions.retrieve(
          row.stripe_subscription_id,
        );
        await syncSubscription(subscription);
        updated += 1;
      } catch (error) {
        failures.push(
          `${row.stripe_subscription_id}: ${
            error instanceof Error ? error.message : String(error)
          }`,
        );
      }
    }
    return json({ updated, failures });
  } catch (error) {
    return errorResponse(error, 500);
  }
});
