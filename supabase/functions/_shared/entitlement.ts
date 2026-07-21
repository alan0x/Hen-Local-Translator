import { admin } from "./clients.ts";
import { decideEntitlement, type Entitlement } from "./entitlement-logic.ts";
export type { Entitlement } from "./entitlement-logic.ts";

export async function entitlementFor(userId: string): Promise<Entitlement> {
  const [
    { data: profile, error: profileError },
    { data: subscription, error: subscriptionError },
  ] = await Promise.all([
    admin.from("profiles").select("trial_started_at").eq("user_id", userId)
      .single(),
    admin.from("subscriptions").select(
      "state,current_period_end,grace_period_end",
    ).eq("user_id", userId).single(),
  ]);
  if (profileError) throw profileError;
  if (subscriptionError) throw subscriptionError;

  return decideEntitlement({
    now: Date.now(),
    trialStartedAt: profile.trial_started_at,
    state: subscription.state,
    currentPeriodEnd: subscription.current_period_end,
    gracePeriodEnd: subscription.grace_period_end,
  });
}

export async function startTrialIfNeeded(userId: string): Promise<void> {
  const { error } = await admin
    .from("profiles")
    .update({
      trial_started_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    })
    .eq("user_id", userId)
    .is("trial_started_at", null);
  if (error) throw error;
}
