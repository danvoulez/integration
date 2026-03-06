-- Migration 017: Normalize llm_requests contract fields (request_id + plan_id + CI/fallback context)
-- Depends on: 007_integration_tables

begin;

alter table llm_requests
  add column if not exists request_id text,
  add column if not exists trace_id text,
  add column if not exists plan_id text,
  add column if not exists ci_target text,
  add column if not exists fallback_behavior text,
  add column if not exists fallback_used boolean not null default false,
  add column if not exists source text not null default 'llm-gateway';

update llm_requests
set
  request_id = coalesce(request_id, id::text),
  trace_id = coalesce(trace_id, request_id, id::text),
  plan_id = coalesce(plan_id, null),
  ci_target = coalesce(ci_target, 'code247-ci/main'),
  fallback_behavior = coalesce(fallback_behavior, 'provider-fallback-with-timeout'),
  source = coalesce(nullif(source, ''), 'llm-gateway');

alter table llm_requests
  alter column request_id set not null,
  alter column trace_id set not null,
  alter column ci_target set not null,
  alter column fallback_behavior set not null;

create index if not exists idx_llm_requests_request_id
  on llm_requests (request_id);
create index if not exists idx_llm_requests_plan_id_created
  on llm_requests (plan_id, created_at desc)
  where plan_id is not null;
create index if not exists idx_llm_requests_provider_mode_created
  on llm_requests (provider, mode, created_at desc);

commit;
