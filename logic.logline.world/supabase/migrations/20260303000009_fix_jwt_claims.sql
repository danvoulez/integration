-- Migration 009: Fix JWT claims extraction for PostgREST compatibility
-- 
-- Problem: Supabase changed how JWT claims are exposed to PostgREST.
-- Old method: request.jwt.claim.<name> (individual settings)
-- New method: request.jwt.claims (single JSON object)
--
-- Our app.current_user_id() only used the old method, causing RLS to fail.
-- This migration updates all app.* claim helpers to check both methods,
-- matching how Supabase's own auth.uid() works.

begin;

-- ─── Updated current_user_id ──────────────────────────────────────────────────
-- Checks claim sources in order:
-- 1. request.jwt.claim.sub (legacy PostgREST)
-- 2. request.jwt.claims->>'sub' (current Supabase PostgREST)
-- 3. app.current_user_id (manual override for service contexts)

create or replace function app.current_user_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      nullif(current_setting('request.jwt.claim.sub', true), ''),
      nullif(current_setting('request.jwt.claims', true)::jsonb ->> 'sub', ''),
      nullif(current_setting('app.current_user_id', true), '')
    ),
    ''
  );
$$;

comment on function app.current_user_id() is
  'Returns the current authenticated user ID from JWT or manual override. '
  'Compatible with both legacy and current Supabase PostgREST claim formats.';

-- ─── Updated current_workspace_id ─────────────────────────────────────────────

create or replace function app.current_workspace_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      nullif(current_setting('request.jwt.claim.workspace_id', true), ''),
      nullif(current_setting('request.jwt.claims', true)::jsonb ->> 'workspace_id', ''),
      nullif(current_setting('app.current_workspace_id', true), '')
    ),
    ''
  );
$$;

comment on function app.current_workspace_id() is
  'Returns the current workspace ID from JWT or manual override.';

-- ─── Updated current_app_id ───────────────────────────────────────────────────

create or replace function app.current_app_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      nullif(current_setting('request.jwt.claim.app_id', true), ''),
      nullif(current_setting('request.jwt.claims', true)::jsonb ->> 'app_id', ''),
      nullif(current_setting('app.current_app_id', true), '')
    ),
    ''
  );
$$;

comment on function app.current_app_id() is
  'Returns the current app ID from JWT or manual override.';

-- ─── New helper: current_tenant_id ────────────────────────────────────────────
-- Service tokens include tenant_id directly in claims

create or replace function app.current_tenant_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      nullif(current_setting('request.jwt.claim.tenant_id', true), ''),
      nullif(current_setting('request.jwt.claims', true)::jsonb ->> 'tenant_id', ''),
      nullif(current_setting('app.current_tenant_id', true), '')
    ),
    ''
  );
$$;

comment on function app.current_tenant_id() is
  'Returns the current tenant ID from JWT (service tokens) or manual override.';

-- ─── New helper: current_role ─────────────────────────────────────────────────
-- Distinguish between 'authenticated' users and 'service' tokens

create or replace function app.current_role()
returns text
language sql
stable
as $$
  select coalesce(
    nullif(current_setting('request.jwt.claim.role', true), ''),
    nullif(current_setting('request.jwt.claims', true)::jsonb ->> 'role', ''),
    'anon'
  );
$$;

comment on function app.current_role() is
  'Returns the current role from JWT: authenticated, service, or anon.';

-- ─── New helper: is_service_token ─────────────────────────────────────────────

create or replace function app.is_service_token()
returns boolean
language sql
stable
as $$
  select app.current_role() = 'service';
$$;

comment on function app.is_service_token() is
  'Returns true if the current request is authenticated via a service token.';

commit;
