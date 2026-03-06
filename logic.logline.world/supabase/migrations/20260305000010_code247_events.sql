-- Migration 010: Code247 execution events ledger
-- Depends on: 007_integration_tables (code247_jobs)

begin;

create table if not exists code247_events (
  event_id      text primary key,
  tenant_id     text not null references tenants(tenant_id),
  app_id        text not null references apps(app_id),
  user_id       text references users(user_id),
  job_id        text references code247_jobs(id) on delete cascade,
  stage         text not null,
  event_type    text not null,
  input         text,
  output        text,
  model_used    text,
  duration_ms   integer,
  occurred_at   timestamptz not null default now(),
  metadata      jsonb not null default '{}'::jsonb
);

create index if not exists idx_code247_events_tenant_app_time
  on code247_events (tenant_id, app_id, occurred_at desc);
create index if not exists idx_code247_events_job_time
  on code247_events (job_id, occurred_at desc);
create index if not exists idx_code247_events_type_time
  on code247_events (event_type, occurred_at desc);

alter table code247_events enable row level security;

drop policy if exists code247_events_select on code247_events;
create policy code247_events_select on code247_events
  for select using (app.is_app_member(tenant_id, app_id));

drop policy if exists code247_events_insert on code247_events;
create policy code247_events_insert on code247_events
  for insert with check (app.is_app_member(tenant_id, app_id));

commit;
