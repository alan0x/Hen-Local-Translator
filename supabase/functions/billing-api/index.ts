import { admin, env, stripe } from "../_shared/clients.ts";
import { requireUser } from "../_shared/auth.ts";
import { entitlementFor } from "../_shared/entitlement.ts";
import { errorResponse, handleOptions, json } from "../_shared/http.ts";

async function customerFor(
  user: { id: string; email?: string },
): Promise<string> {
  const { data: profile, error } = await admin.from("profiles").select(
    "stripe_customer_id",
  ).eq("user_id", user.id).single();
  if (error) throw error;
  if (profile.stripe_customer_id) return profile.stripe_customer_id;
  const customer = await stripe.customers.create({
    email: user.email,
    metadata: { supabase_user_id: user.id },
  });
  const { error: updateError } = await admin.from("profiles").update({
    stripe_customer_id: customer.id,
    updated_at: new Date().toISOString(),
  }).eq("user_id", user.id);
  if (updateError) throw updateError;
  return customer.id;
}

Deno.serve(async (request) => {
  const options = handleOptions(request);
  if (options) return options;
  try {
    const user = await requireUser(request);
    const action = new URL(request.url).pathname.split("/").filter(Boolean)
      .pop();
    if (request.method === "GET" && action === "status") {
      const entitlement = await entitlementFor(user.id);
      const { data: subscription, error } = await admin.from("subscriptions")
        .select("*").eq("user_id", user.id).single();
      if (error) throw error;
      return json({
        entitlement,
        subscription,
        plan: { amountUsd: 49, interval: "month", devices: 2, unlimited: true },
      });
    }
    if (request.method === "POST" && action === "checkout") {
      const customer = await customerFor({ id: user.id, email: user.email });
      const session = await stripe.checkout.sessions.create({
        mode: "subscription",
        customer,
        line_items: [{ price: env("STRIPE_MONTHLY_PRICE_ID"), quantity: 1 }],
        allow_promotion_codes: false,
        success_url: env("STRIPE_CHECKOUT_SUCCESS_URL"),
        cancel_url: env("STRIPE_CHECKOUT_CANCEL_URL"),
        client_reference_id: user.id,
        subscription_data: {
          metadata: { supabase_user_id: user.id, product: "hen-local-monthly" },
        },
        metadata: { supabase_user_id: user.id },
      });
      return json({ url: session.url });
    }
    if (request.method === "POST" && action === "portal") {
      const customer = await customerFor({ id: user.id, email: user.email });
      const session = await stripe.billingPortal.sessions.create({
        customer,
        return_url: env("STRIPE_PORTAL_RETURN_URL"),
      });
      return json({ url: session.url });
    }
    return json({ error: "NOT_FOUND" }, 404);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return errorResponse(error, message.startsWith("AUTH_") ? 401 : 400);
  }
});
