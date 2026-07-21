create extension if not exists pgcrypto;

do $$ begin
  create type public.subscription_state as enum ('trialing', 'active', 'past_due', 'canceled', 'unpaid');
exception when duplicate_object then null;
end $$;

create table if not exists public.profiles (
  user_id uuid primary key references auth.users(id) on delete cascade,
  stripe_customer_id text unique,
  trial_started_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create table if not exists public.subscriptions (
  user_id uuid primary key references auth.users(id) on delete cascade,
  stripe_subscription_id text unique,
  stripe_price_id text,
  state public.subscription_state not null default 'trialing',
  current_period_end timestamptz,
  cancel_at_period_end boolean not null default false,
  grace_period_end timestamptz,
  stripe_event_created_at bigint not null default 0,
  updated_at timestamptz not null default now()
);

create table if not exists public.devices (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users(id) on delete cascade,
  fingerprint text not null,
  public_key text not null,
  friendly_name text not null check (char_length(friendly_name) between 1 and 100),
  activated_at timestamptz not null default now(),
  last_used_at timestamptz not null default now(),
  deactivated_at timestamptz,
  unique (user_id, fingerprint)
);
create index if not exists devices_active_user_idx on public.devices(user_id) where deactivated_at is null;

create table if not exists public.license_leases (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users(id) on delete cascade,
  device_id uuid not null references public.devices(id) on delete cascade,
  issued_at timestamptz not null default now(),
  expires_at timestamptz not null,
  lease_digest text not null,
  revoked_at timestamptz
);
create index if not exists license_leases_device_idx on public.license_leases(device_id, expires_at desc);

create table if not exists public.webhook_events (
  stripe_event_id text primary key,
  event_type text not null,
  received_at timestamptz not null default now(),
  processed_at timestamptz,
  processing_error text
);

create table if not exists public.audit_logs (
  id bigint generated always as identity primary key,
  user_id uuid references auth.users(id) on delete set null,
  device_id uuid references public.devices(id) on delete set null,
  event_type text not null,
  metadata jsonb not null default '{}'::jsonb,
  created_at timestamptz not null default now()
);

alter table public.profiles enable row level security;
alter table public.subscriptions enable row level security;
alter table public.devices enable row level security;
alter table public.license_leases enable row level security;
alter table public.webhook_events enable row level security;
alter table public.audit_logs enable row level security;

drop policy if exists "read own profile" on public.profiles;
create policy "read own profile" on public.profiles for select using (auth.uid() = user_id);
drop policy if exists "read own subscription" on public.subscriptions;
create policy "read own subscription" on public.subscriptions for select using (auth.uid() = user_id);
drop policy if exists "read own devices" on public.devices;
create policy "read own devices" on public.devices for select using (auth.uid() = user_id);

create or replace function public.create_profile_for_new_user()
returns trigger language plpgsql security definer set search_path = '' as $$
begin
  insert into public.profiles(user_id) values (new.id) on conflict do nothing;
  insert into public.subscriptions(user_id, state) values (new.id, 'trialing') on conflict do nothing;
  return new;
end;
$$;

drop trigger if exists on_auth_user_created on auth.users;
create trigger on_auth_user_created after insert on auth.users
for each row execute function public.create_profile_for_new_user();

create or replace function public.activate_device(
  p_user_id uuid,
  p_fingerprint text,
  p_public_key text,
  p_friendly_name text
) returns public.devices
language plpgsql security definer set search_path = public as $$
declare
  result public.devices;
  active_count integer;
begin
  perform pg_advisory_xact_lock(hashtextextended(p_user_id::text, 0));
  select * into result from devices where user_id = p_user_id and fingerprint = p_fingerprint;
  if found then
    update devices set public_key = p_public_key, friendly_name = p_friendly_name,
      deactivated_at = null, last_used_at = now() where id = result.id returning * into result;
  else
    select count(*) into active_count from devices where user_id = p_user_id and deactivated_at is null;
    if active_count >= 2 then raise exception using errcode = 'P0001', message = 'DEVICE_LIMIT_REACHED'; end if;
    insert into devices(user_id, fingerprint, public_key, friendly_name)
      values (p_user_id, p_fingerprint, p_public_key, p_friendly_name) returning * into result;
  end if;
  select count(*) into active_count from devices where user_id = p_user_id and deactivated_at is null;
  if active_count > 2 then raise exception using errcode = 'P0001', message = 'DEVICE_LIMIT_REACHED'; end if;
  return result;
end;
$$;

revoke all on function public.activate_device(uuid, text, text, text) from public, anon, authenticated;
grant execute on function public.activate_device(uuid, text, text, text) to service_role;
