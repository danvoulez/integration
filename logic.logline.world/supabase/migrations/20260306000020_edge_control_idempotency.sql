-- Migration 020: Shared idempotency backend for edge-control
-- Depends on: 001_base_tables

begin;

create table if not exists edge_control_idempotency_keys (
  key           text primary key,
  method        text not null,
  path          text not null,
  owner_app_id  text references apps(app_id) on delete set null,
  first_seen_at timestamptz not null default now(),
  last_seen_at  timestamptz not null default now(),
  expires_at    timestamptz not null,
  created_at    timestamptz not null default now(),
  updated_at    timestamptz not null default now()
);

create index if not exists idx_edge_control_idempotency_expires
  on edge_control_idempotency_keys (expires_at asc);

create index if not exists idx_edge_control_idempotency_owner
  on edge_control_idempotency_keys (owner_app_id, created_at desc);

create or replace function app.update_edge_control_idempotency_timestamp()
returns trigger
language plpgsql
as $$
begin
  new.updated_at = now();
  return new;
end;
$$;

drop trigger if exists trg_edge_control_idempotency_updated on edge_control_idempotency_keys;
create trigger trg_edge_control_idempotency_updated
  before update on edge_control_idempotency_keys
  for each row execute function app.update_edge_control_idempotency_timestamp();

alter table edge_control_idempotency_keys enable row level security;

drop policy if exists edge_control_idempotency_admin_select on edge_control_idempotency_keys;
create policy edge_control_idempotency_admin_select on edge_control_idempotency_keys
  for select using (
    owner_app_id is null
    or app.is_tenant_admin((select tenant_id from apps where app_id = owner_app_id))
    or app.is_app_admin((select tenant_id from apps where app_id = owner_app_id), owner_app_id)
  );

create or replace function app.edge_control_claim_idempotency_key(
  p_key text,
  p_method text,
  p_path text,
  p_ttl_seconds integer default 900,
  p_owner_app_id text default null
)
returns boolean
language plpgsql
security definer
set search_path = app, public
as $$
declare
  v_now timestamptz := now();
  v_expires_at timestamptz;
  v_affected integer := 0;
begin
  if p_key is null or btrim(p_key) = '' then
    return false;
  end if;
  if p_method is null or btrim(p_method) = '' then
    return false;
  end if;
  if p_path is null or btrim(p_path) = '' then
    return false;
  end if;

  v_expires_at := v_now + make_interval(secs => greatest(coalesce(p_ttl_seconds, 900), 60));

  delete from public.edge_control_idempotency_keys
  where expires_at <= v_now;

  insert into public.edge_control_idempotency_keys (
    key,
    method,
    path,
    owner_app_id,
    first_seen_at,
    last_seen_at,
    expires_at
  )
  values (
    btrim(p_key),
    btrim(p_method),
    btrim(p_path),
    nullif(btrim(p_owner_app_id), ''),
    v_now,
    v_now,
    v_expires_at
  )
  on conflict (key) do nothing;

  get diagnostics v_affected = row_count;
  return coalesce(v_affected, 0) > 0;
end;
$$;

create or replace function public.edge_control_claim_idempotency_key(
  p_key text,
  p_method text,
  p_path text,
  p_ttl_seconds integer default 900,
  p_owner_app_id text default null
)
returns boolean
language sql
security definer
set search_path = app, public
as $$
  select app.edge_control_claim_idempotency_key(
    p_key,
    p_method,
    p_path,
    p_ttl_seconds,
    p_owner_app_id
  );
$$;

grant execute on function public.edge_control_claim_idempotency_key(text, text, text, integer, text)
  to authenticated, service_role;

create or replace function app.edge_control_release_idempotency_key(
  p_key text
)
returns boolean
language plpgsql
security definer
set search_path = app, public
as $$
declare
  v_affected integer := 0;
begin
  if p_key is null or btrim(p_key) = '' then
    return false;
  end if;

  delete from public.edge_control_idempotency_keys
  where key = btrim(p_key);

  get diagnostics v_affected = row_count;
  return coalesce(v_affected, 0) > 0;
end;
$$;

create or replace function public.edge_control_release_idempotency_key(
  p_key text
)
returns boolean
language sql
security definer
set search_path = app, public
as $$
  select app.edge_control_release_idempotency_key(p_key);
$$;

grant execute on function public.edge_control_release_idempotency_key(text)
  to authenticated, service_role;

commit;
