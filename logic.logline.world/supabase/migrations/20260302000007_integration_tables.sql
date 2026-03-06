-- Migration 007: Integration tables for llm-gateway and code247
-- Depends on: 004_onboarding (fuel_events table)
-- Referenced by: INTEGRATION_PHASES.md P1-T2

begin;

-- ─── LLM request telemetry ────────────────────────────────────────────────────
-- Links to fuel_events for cost attribution.
-- Used by llm-gateway to log all LLM API calls.

create table if not exists llm_requests (
  id              uuid primary key default gen_random_uuid(),
  fuel_event_id   text references fuel_events(event_id),
  tenant_id       text not null,
  app_id          text not null,
  user_id         text,
  provider        text not null,
  model           text not null,
  mode            text not null default 'auto',
  input_tokens    integer not null default 0,
  output_tokens   integer not null default 0,
  latency_ms      integer not null default 0,
  success         boolean not null default true,
  error_message   text,
  created_at      timestamptz not null default now()
);

create index if not exists idx_llm_requests_tenant 
  on llm_requests (tenant_id, created_at desc);
create index if not exists idx_llm_requests_fuel 
  on llm_requests (fuel_event_id) where fuel_event_id is not null;

-- ─── Code247 jobs ─────────────────────────────────────────────────────────────
-- Tracks autonomous coding jobs submitted via Linear webhook.
-- Replaces local SQLite storage in code247.

create table if not exists code247_jobs (
  id              text primary key,
  tenant_id       text not null references tenants(tenant_id),
  app_id          text not null references apps(app_id),
  user_id         text references users(user_id),
  issue_id        text not null,
  status          text not null default 'PENDING',
  payload         jsonb not null,
  retries         integer not null default 0,
  last_error      text,
  created_at      timestamptz not null default now(),
  updated_at      timestamptz not null default now()
);

create index if not exists idx_code247_jobs_tenant 
  on code247_jobs (tenant_id, status);
create index if not exists idx_code247_jobs_status 
  on code247_jobs (status, created_at);
create index if not exists idx_code247_jobs_issue 
  on code247_jobs (issue_id);

-- ─── Code247 checkpoints ──────────────────────────────────────────────────────
-- Stores state machine checkpoints for job resumption.
-- One checkpoint per (job_id, stage) — upsert pattern.

create table if not exists code247_checkpoints (
  job_id          text not null references code247_jobs(id) on delete cascade,
  stage           text not null,
  data            text not null,
  created_at      timestamptz not null default now(),
  primary key (job_id, stage)
);

-- ─── Trigger: update code247_jobs.updated_at ──────────────────────────────────

create or replace function app.update_code247_job_timestamp()
returns trigger
language plpgsql
as $$
begin
  new.updated_at = now();
  return new;
end;
$$;

drop trigger if exists trg_code247_jobs_updated on code247_jobs;
create trigger trg_code247_jobs_updated
  before update on code247_jobs
  for each row execute function app.update_code247_job_timestamp();

-- ─── RLS policies ─────────────────────────────────────────────────────────────

alter table llm_requests enable row level security;
alter table code247_jobs enable row level security;
alter table code247_checkpoints enable row level security;

-- llm_requests: app admins can read all, app members can read their own
drop policy if exists llm_requests_select_admin on llm_requests;
create policy llm_requests_select_admin on llm_requests
  for select using (
    app.is_app_admin(tenant_id, app_id)
    or app.is_tenant_admin(tenant_id)
  );

drop policy if exists llm_requests_select_self on llm_requests;
create policy llm_requests_select_self on llm_requests
  for select using (
    user_id = (select app.current_user_id())
  );

-- llm_requests: services can insert (via service JWT)
drop policy if exists llm_requests_insert on llm_requests;
create policy llm_requests_insert on llm_requests
  for insert with check (
    app.is_app_member(tenant_id, app_id)
  );

-- code247_jobs: app members can CRUD their jobs
drop policy if exists code247_jobs_select on code247_jobs;
create policy code247_jobs_select on code247_jobs
  for select using (app.is_app_member(tenant_id, app_id));

drop policy if exists code247_jobs_insert on code247_jobs;
create policy code247_jobs_insert on code247_jobs
  for insert with check (app.is_app_member(tenant_id, app_id));

drop policy if exists code247_jobs_update on code247_jobs;
create policy code247_jobs_update on code247_jobs
  for update using (app.is_app_member(tenant_id, app_id));

drop policy if exists code247_jobs_delete on code247_jobs;
create policy code247_jobs_delete on code247_jobs
  for delete using (app.is_app_admin(tenant_id, app_id));

-- code247_checkpoints: access follows parent job
drop policy if exists code247_checkpoints_select on code247_checkpoints;
create policy code247_checkpoints_select on code247_checkpoints
  for select using (
    exists (
      select 1 from code247_jobs j
      where j.id = job_id
        and app.is_app_member(j.tenant_id, j.app_id)
    )
  );

drop policy if exists code247_checkpoints_insert on code247_checkpoints;
create policy code247_checkpoints_insert on code247_checkpoints
  for insert with check (
    exists (
      select 1 from code247_jobs j
      where j.id = job_id
        and app.is_app_member(j.tenant_id, j.app_id)
    )
  );

drop policy if exists code247_checkpoints_update on code247_checkpoints;
create policy code247_checkpoints_update on code247_checkpoints
  for update using (
    exists (
      select 1 from code247_jobs j
      where j.id = job_id
        and app.is_app_member(j.tenant_id, j.app_id)
    )
  );

commit;
