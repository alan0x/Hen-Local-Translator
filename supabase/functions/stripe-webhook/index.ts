import Stripe from "npm:stripe@^22";
import { admin, env, stripe } from "../_shared/clients.ts";
import { errorResponse, json } from "../_shared/http.ts";
import { syncSubscription } from "../_shared/stripe-sync.ts";

const cryptoProvider = Stripe.createSubtleCryptoProvider();

Deno.serve(async (request) => {
  if (request.method !== "POST") {
    return json({ error: "METHOD_NOT_ALLOWED" }, 405);
  }
  const signature = request.headers.get("stripe-signature");
  if (!signature) return json({ error: "MISSING_STRIPE_SIGNATURE" }, 400);
  let event: Stripe.Event;
  try {
    event = await stripe.webhooks.constructEventAsync(
      await request.text(),
      signature,
      env("STRIPE_WEBHOOK_SECRET"),
      undefined,
      cryptoProvider,
    );
  } catch (error) {
    return errorResponse(error, 400);
  }

  const { error: claimError } = await admin.from("webhook_events").insert({
    stripe_event_id: event.id,
    event_type: event.type,
  });
  if (claimError?.code === "23505") {
    return json({ received: true, duplicate: true });
  }
  if (claimError) return errorResponse(claimError, 500);

  try {
    if (event.type.startsWith("customer.subscription.")) {
      await syncSubscription(
        event.data.object as Stripe.Subscription,
        event.created,
      );
    } else if (event.type === "checkout.session.completed") {
      const session = event.data.object as Stripe.Checkout.Session;
      if (session.client_reference_id && typeof session.customer === "string") {
        await admin.from("profiles").update({
          stripe_customer_id: session.customer,
          updated_at: new Date().toISOString(),
        }).eq("user_id", session.client_reference_id);
      }
      if (typeof session.subscription === "string") {
        await syncSubscription(
          await stripe.subscriptions.retrieve(session.subscription),
          event.created,
        );
      }
    } else if (
      event.type === "invoice.payment_failed" || event.type === "invoice.paid"
    ) {
      const invoice = event.data.object as Stripe.Invoice;
      const subscriptionId =
        typeof invoice.parent?.subscription_details?.subscription === "string"
          ? invoice.parent.subscription_details.subscription
          : null;
      if (subscriptionId) {
        await syncSubscription(
          await stripe.subscriptions.retrieve(subscriptionId),
          event.created,
        );
      }
    }
    await admin.from("webhook_events").update({
      processed_at: new Date().toISOString(),
      processing_error: null,
    }).eq("stripe_event_id", event.id);
    return json({ received: true });
  } catch (error) {
    await admin.from("webhook_events").delete().eq("stripe_event_id", event.id);
    return errorResponse(error, 500);
  }
});
