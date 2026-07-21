export type Entitlement = {
  allowed: boolean;
  source: "not_started" | "trial" | "subscription" | "grace" | "expired";
  accessUntil: string;
  subscriptionState: string;
};

export function decideEntitlement(input: {
  now: number;
  trialStartedAt: string | null;
  state: string;
  currentPeriodEnd: string | null;
  gracePeriodEnd: string | null;
}): Entitlement {
  const { now } = input;
  const trialEnd = input.trialStartedAt
    ? new Date(input.trialStartedAt).getTime() + 7 * 24 * 60 * 60 * 1000
    : 0;
  const paidThrough = input.currentPeriodEnd
    ? new Date(input.currentPeriodEnd).getTime()
    : 0;
  const graceEnd = input.gracePeriodEnd
    ? new Date(input.gracePeriodEnd).getTime()
    : 0;

  if (["active", "canceled"].includes(input.state) && paidThrough > now) {
    return {
      allowed: true,
      source: "subscription",
      accessUntil: new Date(paidThrough).toISOString(),
      subscriptionState: input.state,
    };
  }
  if (graceEnd > now) {
    return {
      allowed: true,
      source: "grace",
      accessUntil: new Date(graceEnd).toISOString(),
      subscriptionState: input.state,
    };
  }
  if (trialEnd > now) {
    return {
      allowed: true,
      source: "trial",
      accessUntil: new Date(trialEnd).toISOString(),
      subscriptionState: input.state,
    };
  }
  if (!input.trialStartedAt && !paidThrough && !graceEnd) {
    return {
      allowed: false,
      source: "not_started",
      accessUntil: "",
      subscriptionState: input.state,
    };
  }
  return {
    allowed: false,
    source: "expired",
    accessUntil: new Date(Math.max(trialEnd, paidThrough, graceEnd))
      .toISOString(),
    subscriptionState: input.state,
  };
}
