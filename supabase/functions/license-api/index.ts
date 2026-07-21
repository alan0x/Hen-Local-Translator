import { admin } from "../_shared/clients.ts";
import { requireUser } from "../_shared/auth.ts";
import { entitlementFor, startTrialIfNeeded } from "../_shared/entitlement.ts";
import { errorResponse, handleOptions, json } from "../_shared/http.ts";
import { issueLease } from "../_shared/license.ts";

Deno.serve(async (request) => {
  const options = handleOptions(request);
  if (options) return options;
  try {
    const user = await requireUser(request);
    const action = new URL(request.url).pathname.split("/").filter(Boolean)
      .pop();
    if (request.method === "GET" && action === "devices") {
      const { data, error } = await admin.from("devices").select(
        "id,friendly_name,activated_at,last_used_at,deactivated_at",
      ).eq("user_id", user.id).order("last_used_at", { ascending: false });
      if (error) throw error;
      return json({ devices: data });
    }

    const body = request.method === "POST" ? await request.json() : {};
    if (request.method === "POST" && action === "activate") {
      if (!body.fingerprint || !body.publicKey || !body.friendlyName) {
        throw new Error("DEVICE_FIELDS_REQUIRED");
      }
      await startTrialIfNeeded(user.id);
      const entitlement = await entitlementFor(user.id);
      if (!entitlement.allowed) throw new Error("ENTITLEMENT_EXPIRED");
      const { data, error } = await admin.rpc("activate_device", {
        p_user_id: user.id,
        p_fingerprint: body.fingerprint,
        p_public_key: body.publicKey,
        p_friendly_name: body.friendlyName,
      }).single();
      if (error) throw error;
      const device = data as {
        id: string;
        public_key: string;
        friendly_name: string;
      };
      await admin.from("audit_logs").insert({
        user_id: user.id,
        device_id: device.id,
        event_type: "device_activated",
      });
      return json({
        device,
        entitlement,
        lease: await issueLease(user.id, device, entitlement),
      });
    }
    if (request.method === "POST" && action === "refresh") {
      const { data: device, error } = await admin.from("devices").select("*")
        .eq("id", body.deviceId).eq("user_id", user.id).is(
          "deactivated_at",
          null,
        ).single();
      if (error) throw error;
      const entitlement = await entitlementFor(user.id);
      await admin.from("devices").update({
        last_used_at: new Date().toISOString(),
      }).eq("id", device.id);
      return json({
        entitlement,
        lease: await issueLease(user.id, device, entitlement),
      });
    }
    if (request.method === "POST" && action === "deactivate") {
      const { data: device, error } = await admin.from("devices").update({
        deactivated_at: new Date().toISOString(),
      }).eq("id", body.deviceId).eq("user_id", user.id).select("id").single();
      if (error) throw error;
      await admin.from("license_leases").update({
        revoked_at: new Date().toISOString(),
      }).eq("device_id", device.id).is("revoked_at", null);
      await admin.from("audit_logs").insert({
        user_id: user.id,
        device_id: device.id,
        event_type: "device_deactivated",
      });
      return json({ ok: true });
    }
    return json({ error: "NOT_FOUND" }, 404);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const status = message.startsWith("AUTH_")
      ? 401
      : message.includes("DEVICE_LIMIT_REACHED")
      ? 409
      : message === "ENTITLEMENT_EXPIRED"
      ? 403
      : 400;
    return errorResponse(error, status);
  }
});
