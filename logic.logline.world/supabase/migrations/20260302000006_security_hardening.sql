-- Migration 006: Security hardening for auth hooks
-- Fixes: SEC-002, SEC-003 from audit
-- Depends on: 003_auth_hooks

begin;

-- ─── SEC-002: Revoke direct RPC access to auth hook functions ─────────────────
-- Prevents authenticated users from calling these functions via PostgREST RPC.
-- Auth hooks are only meant to be called by Supabase Auth service internally.

revoke execute on function app.before_user_created(jsonb) from authenticated, anon, public;
revoke execute on function app.custom_access_token(jsonb) from authenticated, anon, public;

-- ─── SEC-003: Grant supabase_auth_admin access to required tables ─────────────
-- Auth hooks need to read these tables but run without SECURITY DEFINER.
-- supabase_auth_admin is the role Supabase Auth uses to call hook functions.

grant usage on schema app to supabase_auth_admin;
grant usage on schema public to supabase_auth_admin;

grant select on tenants to supabase_auth_admin;
grant select on tenant_email_allowlist to supabase_auth_admin;
grant select on tenant_memberships to supabase_auth_admin;
grant select on app_memberships to supabase_auth_admin;

-- Note: provision_user_membership keeps SECURITY DEFINER because it's called
-- by the API route (authenticated role) and needs to write to RLS-protected tables.

commit;
