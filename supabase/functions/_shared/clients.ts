import { createClient } from "npm:@supabase/supabase-js@^2";
import Stripe from "npm:stripe@^22";

function required(name: string): string {
  const value = Deno.env.get(name);
  if (!value) throw new Error(`Missing server secret: ${name}`);
  return value;
}

export const admin = createClient(
  required("SUPABASE_URL"),
  required("SUPABASE_SERVICE_ROLE_KEY"),
  {
    auth: { persistSession: false, autoRefreshToken: false },
  },
);

export const stripe = new Stripe(required("STRIPE_SECRET_KEY"));

export function env(name: string): string {
  return required(name);
}
