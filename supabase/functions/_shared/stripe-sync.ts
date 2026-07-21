import type Stripe from "npm:stripe@^22";
import { admin } from "./clients.ts";

function stateFor(
  status: Stripe.Subscription.Status,
): "trialing" | "active" | "past_due" | "canceled" | "unpaid" {
  if (status === "trialing") return "trialing";
  if (status === "active") return "active";
  if (status === "past_due") return "past_due";
  if (status === "unpaid") return "unpaid";
  return "canceled";
}

export async function syncSubscription(
  subscription: Stripe.Subscription,
  eventCreated = Math.floor(Date.now() / 1000),
): Promise<void> {
  const customerId = typeof subscription.customer === "string"
    ? subscription.customer
    : subscription.customer.id;
  const { data: profile, error: profileError } = await admin.from("profiles")
    .select("user_id").eq("stripe_customer_id", customerId).single();
  if (profileError) throw profileError;
  const item = subscription.items.data[0];
  const periodEnd = item?.current_period_end ??
    (subscription as unknown as { current_period_end?: number })
      .current_period_end;
  const state = stateFor(subscription.status);
  const { data: existing } = await admin
    .from("subscriptions")
    .select("stripe_event_created_at,state,grace_period_end")
    .eq("user_id", profile.user_id)
    .single();
  if ((existing?.stripe_event_created_at ?? 0) > eventCreated) return;
  const gracePeriodEnd = state === "past_due"
    ? existing?.state === "past_due" && existing.grace_period_end
      ? existing.grace_period_end
      : new Date(Date.now() + 3 * 24 * 60 * 60 * 1000).toISOString()
    : null;
  const row = {
    user_id: profile.user_id,
    stripe_subscription_id: subscription.id,
    stripe_price_id: item?.price?.id ?? null,
    state,
    current_period_end: periodEnd
      ? new Date(periodEnd * 1000).toISOString()
      : null,
    cancel_at_period_end: subscription.cancel_at_period_end,
    grace_period_end: gracePeriodEnd,
    stripe_event_created_at: eventCreated,
    updated_at: new Date().toISOString(),
  };
  const { error } = await admin.from("subscriptions").upsert(row, {
    onConflict: "user_id",
  });
  if (error) throw error;
}
