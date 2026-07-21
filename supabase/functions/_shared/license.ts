import { admin, env } from "./clients.ts";
import type { Entitlement } from "./entitlement.ts";

function fromBase64(value: string): Uint8Array {
  return Uint8Array.from(
    atob(value.replace(/\s/g, "")),
    (character) => character.charCodeAt(0),
  );
}

function toBase64(value: ArrayBuffer): string {
  const bytes = new Uint8Array(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

export async function issueLease(
  userId: string,
  device: { id: string; public_key: string },
  entitlement: Entitlement,
) {
  if (!entitlement.allowed) throw new Error("ENTITLEMENT_EXPIRED");
  const issuedAt = new Date();
  const sevenDays = issuedAt.getTime() + 7 * 24 * 60 * 60 * 1000;
  const expiresAt = new Date(
    Math.min(sevenDays, new Date(entitlement.accessUntil).getTime()),
  );
  const payload = {
    version: 1,
    keyId: env("LICENSE_KEY_ID"),
    userId,
    deviceId: device.id,
    devicePublicKey: device.public_key,
    issuedAt: issuedAt.toISOString(),
    expiresAt: expiresAt.toISOString(),
    plan: "hen-local-monthly",
    unlimitedLocalTranslation: true,
    source: entitlement.source,
  };
  const canonical = JSON.stringify(payload);
  const privateKey = await crypto.subtle.importKey(
    "pkcs8",
    fromBase64(env("LICENSE_PRIVATE_KEY_PKCS8_BASE64")),
    { name: "Ed25519" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign(
    "Ed25519",
    privateKey,
    new TextEncoder().encode(canonical),
  );
  const digest = toBase64(
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(canonical)),
  );
  const { error } = await admin.from("license_leases").insert({
    user_id: userId,
    device_id: device.id,
    expires_at: expiresAt.toISOString(),
    lease_digest: digest,
  });
  if (error) throw error;
  return { payload, signature: toBase64(signature) };
}
