-- Migration 008: Service tokens for long-lived app-to-app authentication
-- Enables ecosystem apps (code247, obs-api) to call other services (llm-gateway)
-- with delegated billing attribution to the original tenant/user.
--
-- Depends on: 001_base_tables, 002_rbac_rls, 004_onboarding

begin;

-- ─── 1. Service tokens table ─────────────────────────────────────────────────
-- Stores metadata about issued service tokens (JWTs).
-- The actual JWT is returned to the caller and not stored here.
-- token_hash allows revocation without storing the secret.

create table if not exists service_tokens (
  token_id      text primary key default gen_random_uuid()::text,
  app_id        text not null references apps(app_id) on delete cascade,
  tenant_id     text not null references tenants(tenant_id) on delete cascade,
  token_hash    text not null,                    -- SHA256 of the JWT for revocation lookup
  capabilities  jsonb not null default '[]',     -- ["llm:call", "fuel:emit", ...]
  issued_by     text references users(user_id),
  issued_at     timestamptz not null default now(),
  expires_at    timestamptz not null,
  revoked_at    timestamptz,
  revoked_by    text references users(user_id),
  last_used_at  timestamptz,
  use_count     integer not null default 0,
  description   text                              -- human-readable note
);

-- Indexes for common queries
create index if not exists idx_service_tokens_app_tenant 
  on service_tokens (app_id, tenant_id) where revoked_at is null;
create index if not exists idx_service_tokens_hash 
  on service_tokens (token_hash);
create index if not exists idx_service_tokens_expires 
  on service_tokens (expires_at) where revoked_at is null;

-- ─── 2. RLS policies ─────────────────────────────────────────────────────────

alter table service_tokens enable row level security;

-- App admins can view their app's tokens
drop policy if exists service_tokens_select on service_tokens;
create policy service_tokens_select on service_tokens
  for select using (
    app.is_app_admin(tenant_id, app_id)
    or app.is_tenant_admin(tenant_id)
  );

-- App admins can insert tokens for their app
drop policy if exists service_tokens_insert on service_tokens;
create policy service_tokens_insert on service_tokens
  for insert with check (
    app.is_app_admin(tenant_id, app_id)
  );

-- App admins can update (revoke) their app's tokens
drop policy if exists service_tokens_update on service_tokens;
create policy service_tokens_update on service_tokens
  for update using (
    app.is_app_admin(tenant_id, app_id)
    or app.is_tenant_admin(tenant_id)
  );

-- No delete - tokens are revoked, not deleted (audit trail)
revoke delete on service_tokens from authenticated, anon, public;

-- ─── 3. Helper functions ─────────────────────────────────────────────────────

-- Check if a service token is valid (not expired, not revoked)
create or replace function app.is_service_token_valid(p_token_hash text)
returns boolean
language sql
stable
as $$
  select exists (
    select 1 from service_tokens
    where token_hash = p_token_hash
      and revoked_at is null
      and expires_at > now()
  );
$$;

-- Record token usage (called by services that validate tokens)
create or replace function app.record_token_usage(p_token_hash text)
returns void
language sql
as $$
  update service_tokens
  set last_used_at = now(),
      use_count = use_count + 1
  where token_hash = p_token_hash;
$$;

-- Revoke a token
create or replace function app.revoke_service_token(
  p_token_id text,
  p_revoked_by text default null
)
returns boolean
language plpgsql
as $$
begin
  update service_tokens
  set revoked_at = now(),
      revoked_by = coalesce(p_revoked_by, current_setting('app.current_user_id', true))
  where token_id = p_token_id
    and revoked_at is null;
  return found;
end;
$$;

-- ─── 4. Trusted apps configuration ───────────────────────────────────────────
-- Apps listed here can use x-on-behalf-of-* headers for billing delegation

create table if not exists trusted_apps (
  app_id        text primary key references apps(app_id) on delete cascade,
  trust_level   text not null default 'standard' 
                check (trust_level in ('standard', 'elevated', 'system')),
  can_delegate  boolean not null default false,  -- can use x-on-behalf-of-*
  granted_at    timestamptz not null default now(),
  granted_by    text references users(user_id),
  notes         text
);

alter table trusted_apps enable row level security;

-- Only tenant admins can manage trusted apps
drop policy if exists trusted_apps_select on trusted_apps;
create policy trusted_apps_select on trusted_apps
  for select using (true);  -- anyone can see which apps are trusted

drop policy if exists trusted_apps_write on trusted_apps;
create policy trusted_apps_write on trusted_apps
  for all using (
    app.is_tenant_admin((select tenant_id from apps where app_id = trusted_apps.app_id))
  );

-- ─── 5. Useful views ─────────────────────────────────────────────────────────

create or replace view v_active_service_tokens as
select 
  t.token_id,
  t.app_id,
  t.tenant_id,
  a.name as app_name,
  t.capabilities,
  t.issued_at,
  t.expires_at,
  t.last_used_at,
  t.use_count,
  t.description,
  t.expires_at - now() as time_remaining
from service_tokens t
join apps a on a.app_id = t.app_id
where t.revoked_at is null
  and t.expires_at > now();

commit;
