-- Migration 019: Fuel policy segmentation, calibration views, alert candidates and ops job evidence
-- Depends on: 011_fuel_valuation_points, 012_fuel_l0_reconciler, 016_fuel_window_baseline, 017_llm_requests_normalized_contract

begin;

create table if not exists fuel_policy_assignments (
  id             bigserial primary key,
  tenant_id      text not null references tenants(tenant_id) on delete cascade,
  app_id         text references apps(app_id) on delete cascade,
  policy_version text not null references fuel_control_policy_versions(policy_version),
  effective_from timestamptz not null default now(),
  effective_to   timestamptz,
  notes          text,
  created_at     timestamptz not null default now(),
  updated_at     timestamptz not null default now(),
  check (effective_to is null or effective_to > effective_from)
);

create index if not exists idx_fuel_policy_assignments_lookup
  on fuel_policy_assignments (tenant_id, app_id, effective_from desc);

create unique index if not exists idx_fuel_policy_assignments_unique_window
  on fuel_policy_assignments (tenant_id, coalesce(app_id, ''), policy_version, effective_from);

create or replace function app.update_fuel_policy_assignment_timestamp()
returns trigger
language plpgsql
as $$
begin
  new.updated_at = now();
  return new;
end;
$$;

drop trigger if exists trg_fuel_policy_assignments_updated on fuel_policy_assignments;
create trigger trg_fuel_policy_assignments_updated
  before update on fuel_policy_assignments
  for each row execute function app.update_fuel_policy_assignment_timestamp();

alter table fuel_policy_assignments enable row level security;

drop policy if exists fuel_policy_assignments_select on fuel_policy_assignments;
create policy fuel_policy_assignments_select on fuel_policy_assignments
  for select using (
    app.is_tenant_admin(tenant_id)
    or (app_id is not null and app.is_app_admin(tenant_id, app_id))
  );

drop policy if exists fuel_policy_assignments_mutate_admin on fuel_policy_assignments;
create policy fuel_policy_assignments_mutate_admin on fuel_policy_assignments
  for all using (app.current_user_id() = 'founder')
  with check (app.current_user_id() = 'founder');

create table if not exists fuel_ops_job_runs (
  id             bigserial primary key,
  job_name       text not null check (job_name in ('baseline_snapshot', 'alerts_snapshot', 'baseline_and_alerts')),
  evidence_date  date not null,
  tenant_id      text,
  app_id         text,
  policy_version text,
  status         text not null check (status in ('ok', 'warn', 'failed')),
  started_at     timestamptz not null default now(),
  completed_at   timestamptz,
  metrics        jsonb not null default '{}'::jsonb,
  evidence       jsonb not null default '{}'::jsonb,
  created_at     timestamptz not null default now()
);

create unique index if not exists idx_fuel_ops_job_runs_unique_day
  on fuel_ops_job_runs (job_name, evidence_date, coalesce(tenant_id, ''), coalesce(app_id, ''), coalesce(policy_version, ''));

create index if not exists idx_fuel_ops_job_runs_created
  on fuel_ops_job_runs (evidence_date desc, created_at desc);

alter table fuel_ops_job_runs enable row level security;

drop policy if exists fuel_ops_job_runs_select on fuel_ops_job_runs;
create policy fuel_ops_job_runs_select on fuel_ops_job_runs
  for select using (
    tenant_id is null
    or app.is_tenant_admin(tenant_id)
    or (app_id is not null and app.is_app_member(tenant_id, app_id))
  );

drop policy if exists fuel_ops_job_runs_mutate_admin on fuel_ops_job_runs;
create policy fuel_ops_job_runs_mutate_admin on fuel_ops_job_runs
  for all using (app.current_user_id() = 'founder')
  with check (app.current_user_id() = 'founder');

-- Older environments may already have the pre-segmentation Fuel views with a
-- different column layout. Recreate the chain explicitly to avoid signature
-- mismatch errors during CREATE OR REPLACE VIEW.
drop view if exists fuel_alert_candidates_v1;
drop view if exists fuel_policy_calibration_daily_v1;
drop view if exists fuel_llm_mode_provider_metrics_v1;
drop view if exists fuel_policy_precision_daily_v1;
drop view if exists fuel_window_realtime_v1;
drop view if exists fuel_window_baseline_v1;
drop view if exists fuel_window_metrics_v1;
drop view if exists fuel_valuation_drift_v1;
drop view if exists fuel_points_v1;

create or replace view fuel_points_v1 as
with default_policy as (
  select policy_version, slo_ttft_ms, slo_latency_ms, k_latency, k_errors, k_energy
  from fuel_control_policy_versions
  order by updated_at desc
  limit 1
),
enriched as (
  select
    fe.event_id,
    fe.occurred_at,
    fe.tenant_id,
    fe.app_id,
    fe.user_id,
    fe.source,
    fe.unit_type,
    fe.units as quantity,
    fe.metadata,
    fe.metadata->>'event_type' as event_type,
    coalesce(fe.metadata->>'trace_id', fe.event_id) as trace_id,
    fe.metadata->>'parent_event_id' as parent_event_id,
    coalesce(fe.metadata->>'outcome', 'ok') as outcome,
    coalesce(fe.metadata->>'provider', 'unknown') as provider,
    coalesce(fe.metadata->>'model', 'unknown') as model,
    coalesce(fe.metadata->>'mode', 'unknown') as mode,
    app.to_numeric_or_zero(fe.metadata->>'prompt_tokens') as prompt_tokens,
    app.to_numeric_or_zero(fe.metadata->>'completion_tokens') as completion_tokens,
    coalesce(fv.usd_settled, fv.usd_estimated, 0) as usd_effective,
    coalesce(fv.usd_estimated, 0) as usd_estimated,
    fv.usd_settled,
    coalesce(fv.energy_kwh, 0) as energy_kwh,
    coalesce(fv.carbon_gco2e, 0) as carbon_gco2e,
    coalesce(fv.price_card_version, 'unpriced') as price_card_version,
    coalesce(fv.valuation_version, 'fuel-valuation.v1') as valuation_version,
    coalesce(fv.precision_level, 'L0') as precision_level,
    coalesce(fv.confidence, 0) as confidence,
    app.to_numeric_or_zero(fe.metadata->>'latency_ms') as latency_ms,
    app.to_numeric_or_zero(fe.metadata->>'ttft_ms') as ttft_ms,
    app.to_numeric_or_zero(fe.metadata->>'retry_count') as retry_count,
    case
      when lower(coalesce(fe.metadata->>'fallback_used', 'false')) in ('true', '1', 'yes') then 1
      else 0
    end as fallback_used,
    case
      when lower(coalesce(fe.metadata->>'error_message', '')) like '%timeout%'
        or lower(coalesce(fe.metadata->>'error_message', '')) like '%timed out%'
      then 1
      else 0
    end as timeout_like
  from fuel_events fe
  left join fuel_valuations fv on fv.event_id = fe.event_id
),
scored as (
  select
    e.*,
    coalesce(pa.policy_version, dp.policy_version, 'fuel-policy.v1') as policy_version,
    coalesce(pa.slo_ttft_ms, dp.slo_ttft_ms, 1200) as slo_ttft_ms,
    coalesce(pa.slo_latency_ms, dp.slo_latency_ms, 1500) as slo_latency_ms,
    coalesce(pa.k_latency, dp.k_latency, 0.50)::numeric as k_latency,
    coalesce(pa.k_errors, dp.k_errors, 1.00)::numeric as k_errors,
    coalesce(pa.k_energy, dp.k_energy, 0.25)::numeric as k_energy,
    (e.usd_effective * 1000.0) as base_cost_points,
    greatest(
      0,
      (
        greatest(e.latency_ms, e.ttft_ms)
        - greatest(
          coalesce(pa.slo_latency_ms, dp.slo_latency_ms, 1500)::numeric,
          coalesce(pa.slo_ttft_ms, dp.slo_ttft_ms, 1200)::numeric
        )
      ) / nullif(
        greatest(
          coalesce(pa.slo_latency_ms, dp.slo_latency_ms, 1500)::numeric,
          coalesce(pa.slo_ttft_ms, dp.slo_ttft_ms, 1200)::numeric
        ),
        0
      )
    ) as latency_excess_ratio,
    case when e.outcome <> 'ok' then coalesce(pa.k_errors, dp.k_errors, 1.00)::numeric else 0::numeric end as error_k_component
  from enriched e
  cross join default_policy dp
  left join lateral (
    select
      fpa.policy_version,
      fcpv.slo_ttft_ms,
      fcpv.slo_latency_ms,
      fcpv.k_latency,
      fcpv.k_errors,
      fcpv.k_energy
    from fuel_policy_assignments fpa
    join fuel_control_policy_versions fcpv on fcpv.policy_version = fpa.policy_version
    where fpa.tenant_id = e.tenant_id
      and (fpa.app_id is null or fpa.app_id = e.app_id)
      and e.occurred_at >= fpa.effective_from
      and (fpa.effective_to is null or e.occurred_at < fpa.effective_to)
    order by case when fpa.app_id = e.app_id then 0 else 1 end, fpa.effective_from desc
    limit 1
  ) pa on true
)
select
  event_id,
  occurred_at,
  tenant_id,
  app_id,
  user_id,
  source,
  unit_type,
  quantity,
  event_type,
  trace_id,
  parent_event_id,
  outcome,
  provider,
  model,
  mode,
  prompt_tokens,
  completion_tokens,
  usd_estimated,
  usd_settled,
  usd_effective,
  energy_kwh,
  carbon_gco2e,
  price_card_version,
  valuation_version,
  precision_level,
  confidence,
  policy_version,
  slo_ttft_ms,
  slo_latency_ms,
  k_latency,
  k_errors,
  k_energy,
  base_cost_points,
  latency_ms,
  ttft_ms,
  retry_count,
  fallback_used,
  timeout_like,
  latency_excess_ratio,
  error_k_component,
  (latency_excess_ratio * k_latency) as penalty_latency,
  (error_k_component + retry_count * 0.20 + fallback_used * 0.50) as penalty_errors,
  (energy_kwh * k_energy) as penalty_energy,
  (
    base_cost_points
    * (
      1
      + (latency_excess_ratio * k_latency)
      + (error_k_component + retry_count * 0.20 + fallback_used * 0.50)
      + (energy_kwh * k_energy)
    )
  ) as fuel_points_total,
  now() as computed_at
from scored;

create or replace view fuel_valuation_drift_v1 as
select
  date_trunc('day', occurred_at) as day,
  tenant_id,
  app_id,
  source,
  policy_version,
  precision_level,
  count(*)::bigint as event_count,
  sum(coalesce(usd_estimated, 0))::numeric as usd_estimated_total,
  sum(coalesce(usd_settled, 0))::numeric as usd_settled_total,
  sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
  sum(coalesce(usd_settled, 0) - coalesce(usd_estimated, 0))::numeric as usd_drift_total
from fuel_points_v1
group by 1, 2, 3, 4, 5, 6;

create or replace view fuel_window_metrics_v1 as
with base as (
  select
    to_timestamp(floor(extract(epoch from fp.occurred_at) / 300) * 300)::timestamptz as window_start,
    fp.tenant_id,
    fp.app_id,
    fp.source,
    fp.policy_version,
    count(*)::bigint as event_count,
    sum(coalesce(fp.fuel_points_total, 0))::numeric as fuel_points_total,
    sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
    sum(coalesce(fp.usd_estimated, 0))::numeric as usd_estimated_total,
    sum(coalesce(fp.usd_settled, 0))::numeric as usd_settled_total,
    sum(coalesce(fp.energy_kwh, 0))::numeric as energy_kwh_total,
    sum(case when coalesce(fp.outcome, 'ok') <> 'ok' then 1 else 0 end)::bigint as error_count,
    sum(case when coalesce(fp.fallback_used, 0) > 0 then 1 else 0 end)::bigint as fallback_count,
    sum(case when coalesce(fp.timeout_like, 0) > 0 then 1 else 0 end)::bigint as timeout_count,
    percentile_cont(0.95) within group (order by fp.latency_ms)
      filter (where fp.latency_ms > 0)::numeric as latency_p95_ms,
    percentile_cont(0.95) within group (order by fp.ttft_ms)
      filter (where fp.ttft_ms > 0)::numeric as ttft_p95_ms
  from fuel_points_v1 fp
  group by 1, 2, 3, 4, 5
)
select
  window_start,
  tenant_id,
  app_id,
  source,
  policy_version,
  event_count,
  fuel_points_total,
  usd_effective_total,
  usd_estimated_total,
  usd_settled_total,
  energy_kwh_total,
  error_count,
  fallback_count,
  timeout_count,
  case when event_count > 0 then error_count::numeric / event_count::numeric else 0 end as error_rate,
  case when event_count > 0 then fallback_count::numeric / event_count::numeric else 0 end as fallback_rate,
  case when event_count > 0 then timeout_count::numeric / event_count::numeric else 0 end as timeout_rate,
  latency_p95_ms,
  ttft_p95_ms,
  extract(isodow from timezone('UTC', window_start))::int as iso_dow,
  extract(hour from timezone('UTC', window_start))::int as hour_utc,
  floor(extract(minute from timezone('UTC', window_start)) / 5)::int as minute_bucket_5,
  now() as computed_at
from base;

create or replace view fuel_window_baseline_v1 as
select
  tenant_id,
  app_id,
  source,
  policy_version,
  iso_dow,
  hour_utc,
  minute_bucket_5,
  count(*)::bigint as sample_count,
  percentile_cont(0.50) within group (order by fuel_points_total)::numeric as fuel_points_p50,
  percentile_cont(0.75) within group (order by fuel_points_total)::numeric as fuel_points_p75,
  percentile_cont(0.95) within group (order by fuel_points_total)::numeric as fuel_points_p95,
  percentile_cont(0.50) within group (order by usd_effective_total)::numeric as usd_effective_p50,
  percentile_cont(0.75) within group (order by usd_effective_total)::numeric as usd_effective_p75,
  percentile_cont(0.95) within group (order by usd_effective_total)::numeric as usd_effective_p95,
  percentile_cont(0.95) within group (order by error_rate)::numeric as error_rate_p95,
  percentile_cont(0.95) within group (order by fallback_rate)::numeric as fallback_rate_p95,
  percentile_cont(0.95) within group (order by timeout_rate)::numeric as timeout_rate_p95,
  percentile_cont(0.95) within group (order by coalesce(latency_p95_ms, 0))::numeric as latency_p95_envelope,
  percentile_cont(0.95) within group (order by coalesce(ttft_p95_ms, 0))::numeric as ttft_p95_envelope,
  min(window_start) as baseline_window_from,
  max(window_start) as baseline_window_to,
  now() as computed_at
from fuel_window_metrics_v1
where window_start >= now() - interval '60 days'
  and window_start < now() - interval '5 minutes'
group by 1, 2, 3, 4, 5, 6, 7
having count(*) >= 8;

create or replace view fuel_window_realtime_v1 as
select
  wm.window_start,
  wm.tenant_id,
  wm.app_id,
  wm.source,
  wm.policy_version,
  wm.event_count,
  wm.fuel_points_total,
  wm.usd_effective_total,
  wm.usd_estimated_total,
  wm.usd_settled_total,
  wm.energy_kwh_total,
  wm.error_count,
  wm.fallback_count,
  wm.timeout_count,
  wm.error_rate,
  wm.fallback_rate,
  wm.timeout_rate,
  wm.latency_p95_ms,
  wm.ttft_p95_ms,
  wm.iso_dow,
  wm.hour_utc,
  wm.minute_bucket_5,
  wb.sample_count,
  wb.fuel_points_p50,
  wb.fuel_points_p75,
  wb.fuel_points_p95,
  wb.usd_effective_p50,
  wb.usd_effective_p75,
  wb.usd_effective_p95,
  wb.error_rate_p95,
  wb.fallback_rate_p95,
  wb.timeout_rate_p95,
  wb.latency_p95_envelope,
  wb.ttft_p95_envelope,
  case
    when wb.fuel_points_p50 is null or wb.fuel_points_p50 = 0 then null
    else wm.fuel_points_total / wb.fuel_points_p50
  end as fuel_points_ratio_to_p50,
  case
    when wb.fuel_points_p95 is null or wb.fuel_points_p95 = 0 then null
    else wm.fuel_points_total / wb.fuel_points_p95
  end as fuel_points_ratio_to_p95,
  case
    when wb.fuel_points_p95 is null then 'cold_start'
    when wm.fuel_points_total >= wb.fuel_points_p95 then 'above_p95'
    when wm.fuel_points_total >= wb.fuel_points_p75 then 'above_p75'
    else 'within_envelope'
  end as envelope_status,
  now() as computed_at
from fuel_window_metrics_v1 wm
left join fuel_window_baseline_v1 wb
  on wb.tenant_id = wm.tenant_id
 and wb.app_id = wm.app_id
 and wb.source = wm.source
 and wb.policy_version = wm.policy_version
 and wb.iso_dow = wm.iso_dow
 and wb.hour_utc = wm.hour_utc
 and wb.minute_bucket_5 = wm.minute_bucket_5;

create or replace view fuel_policy_precision_daily_v1 as
select
  date_trunc('day', occurred_at) as day,
  tenant_id,
  app_id,
  policy_version,
  precision_level,
  count(*)::bigint as event_count,
  sum(case when usd_settled is not null then 1 else 0 end)::bigint as settled_count,
  sum(case when fallback_used > 0 then 1 else 0 end)::bigint as fallback_count,
  sum(case when timeout_like > 0 then 1 else 0 end)::bigint as timeout_count,
  sum(case when outcome <> 'ok' then 1 else 0 end)::bigint as error_count,
  sum(coalesce(usd_estimated, 0))::numeric as usd_estimated_total,
  sum(coalesce(usd_settled, 0))::numeric as usd_settled_total,
  sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
  sum(coalesce(energy_kwh, 0))::numeric as energy_kwh_total,
  sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
  avg(coalesce(confidence, 0))::numeric as confidence_avg
from fuel_points_v1
group by 1, 2, 3, 4, 5;

create or replace view fuel_llm_mode_provider_metrics_v1 as
select
  date_trunc('day', lr.created_at) as day,
  lr.tenant_id,
  lr.app_id,
  coalesce(fp.policy_version, 'unassigned') as policy_version,
  coalesce(fp.precision_level, 'L0') as precision_level,
  coalesce(lr.mode, 'unknown') as mode,
  coalesce(lr.provider, 'unknown') as provider,
  coalesce(lr.model, 'unknown') as model,
  count(*)::bigint as request_count,
  sum(case when lr.success then 1 else 0 end)::bigint as success_count,
  sum(case when lr.success then 0 else 1 end)::bigint as failure_count,
  sum(case when coalesce(lr.fallback_used, false) then 1 else 0 end)::bigint as fallback_count,
  sum(case when lower(coalesce(lr.error_message, '')) like '%timeout%' then 1 else 0 end)::bigint as timeout_count,
  sum(coalesce(lr.input_tokens, 0))::bigint as prompt_tokens_total,
  sum(coalesce(lr.output_tokens, 0))::bigint as completion_tokens_total,
  sum(coalesce(lr.input_tokens, 0) + coalesce(lr.output_tokens, 0))::bigint as tokens_total,
  avg(coalesce(lr.latency_ms, 0))::numeric as latency_avg_ms,
  percentile_cont(0.95) within group (order by coalesce(lr.latency_ms, 0))::numeric as latency_p95_ms,
  percentile_cont(0.99) within group (order by coalesce(lr.latency_ms, 0))::numeric as latency_p99_ms,
  sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
  sum(coalesce(fp.fuel_points_total, 0))::numeric as fuel_points_total,
  sum(coalesce(fp.energy_kwh, 0))::numeric as energy_kwh_total,
  case
    when sum(coalesce(lr.input_tokens, 0) + coalesce(lr.output_tokens, 0)) > 0 then
      (sum(coalesce(fp.usd_effective, 0)) * 1000.0)
      / sum(coalesce(lr.input_tokens, 0) + coalesce(lr.output_tokens, 0))
    else 0::numeric
  end as usd_effective_per_1k_tokens,
  case
    when count(*) > 0 then sum(case when fp.usd_settled is not null then 1 else 0 end)::numeric / count(*)::numeric
    else 0::numeric
  end as settled_ratio
from llm_requests lr
left join fuel_points_v1 fp on fp.event_id = lr.fuel_event_id
group by 1, 2, 3, 4, 5, 6, 7, 8;

create or replace view fuel_policy_calibration_daily_v1 as
select
  date_trunc('day', occurred_at) as day,
  tenant_id,
  app_id,
  policy_version,
  count(*)::bigint as event_count,
  max(k_latency)::numeric as k_latency_current,
  max(k_errors)::numeric as k_errors_current,
  max(k_energy)::numeric as k_energy_current,
  sum(coalesce(base_cost_points, 0))::numeric as base_cost_points_total,
  sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
  sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
  sum(coalesce(penalty_latency, 0))::numeric as penalty_latency_total,
  sum(coalesce(penalty_errors, 0))::numeric as penalty_errors_total,
  sum(coalesce(penalty_energy, 0))::numeric as penalty_energy_total,
  percentile_cont(0.95) within group (order by coalesce(penalty_latency, 0))::numeric as penalty_latency_p95,
  percentile_cont(0.95) within group (order by coalesce(penalty_errors, 0))::numeric as penalty_errors_p95,
  percentile_cont(0.95) within group (order by coalesce(penalty_energy, 0))::numeric as penalty_energy_p95,
  percentile_cont(0.95) within group (order by coalesce(latency_excess_ratio, 0))::numeric as latency_excess_ratio_p95,
  percentile_cont(0.95) within group (order by coalesce(error_k_component + retry_count * 0.20 + fallback_used * 0.50, 0))::numeric as error_signal_p95,
  percentile_cont(0.95) within group (order by coalesce(energy_kwh, 0))::numeric as energy_kwh_p95,
  sum(coalesce(base_cost_points, 0) * coalesce(penalty_latency, 0) * 0.10)::numeric as delta_points_if_k_latency_plus_10pct,
  sum(coalesce(base_cost_points, 0) * coalesce(error_k_component, 0) * 0.10)::numeric as delta_points_if_k_errors_plus_10pct,
  sum(coalesce(base_cost_points, 0) * coalesce(penalty_energy, 0) * 0.10)::numeric as delta_points_if_k_energy_plus_10pct,
  case
    when sum(coalesce(fuel_points_total, 0)) > 0 then sum(coalesce(penalty_latency, 0)) / sum(coalesce(fuel_points_total, 0))
    else 0::numeric
  end as sensitivity_latency_share,
  case
    when sum(coalesce(fuel_points_total, 0)) > 0 then sum(coalesce(penalty_errors, 0)) / sum(coalesce(fuel_points_total, 0))
    else 0::numeric
  end as sensitivity_errors_share,
  case
    when sum(coalesce(fuel_points_total, 0)) > 0 then sum(coalesce(penalty_energy, 0)) / sum(coalesce(fuel_points_total, 0))
    else 0::numeric
  end as sensitivity_energy_share
from fuel_points_v1
group by 1, 2, 3, 4;

create or replace view fuel_alert_candidates_v1 as
with recent_hour as (
  select
    tenant_id,
    app_id,
    policy_version,
    count(*)::bigint as event_count,
    avg(coalesce(penalty_latency, 0))::numeric as latency_penalty_avg,
    percentile_cont(0.95) within group (order by coalesce(penalty_latency, 0))::numeric as latency_penalty_p95,
    avg(case when outcome <> 'ok' then 1 else 0 end)::numeric as error_rate,
    avg(case when fallback_used > 0 then 1 else 0 end)::numeric as fallback_rate,
    avg(case when timeout_like > 0 then 1 else 0 end)::numeric as timeout_rate,
    sum(coalesce(penalty_energy, 0))::numeric as energy_penalty_total,
    sum(coalesce(fuel_points_total, 0))::numeric as fuel_points_total,
    avg(coalesce(confidence, 0))::numeric as confidence_avg,
    max(occurred_at) as latest_at
  from fuel_points_v1
  where occurred_at >= now() - interval '60 minutes'
  group by 1, 2, 3
),
coverage_day as (
  select
    tenant_id,
    app_id,
    policy_version,
    count(*)::bigint as event_count,
    sum(case when precision_level in ('L0', 'L1') then 1 else 0 end)::bigint as l0_l1_count,
    sum(case when precision_level = 'L3' then 1 else 0 end)::bigint as l3_count,
    avg(coalesce(confidence, 0))::numeric as confidence_avg,
    max(occurred_at) as latest_at
  from fuel_points_v1
  where occurred_at >= now() - interval '24 hours'
  group by 1, 2, 3
),
drift_day as (
  select
    tenant_id,
    app_id,
    policy_version,
    sum(coalesce(usd_effective, 0))::numeric as usd_effective_total,
    sum(abs(coalesce(usd_settled, 0) - coalesce(usd_estimated, 0)))::numeric as usd_drift_abs_total,
    count(*)::bigint as event_count,
    max(occurred_at) as latest_at
  from fuel_points_v1
  where occurred_at >= now() - interval '24 hours'
  group by 1, 2, 3
)
select
  tenant_id,
  app_id,
  policy_version,
  null::text as precision_level,
  'fuel.latency.pressure'::text as alert_code,
  case when latency_penalty_p95 >= 0.50 then 'critical' else 'warn' end as severity,
  latest_at as detected_at,
  format('Latency pressure above threshold for %s/%s (%s)', tenant_id, app_id, policy_version) as summary,
  jsonb_build_object(
    'window', '60m',
    'event_count', event_count,
    'latency_penalty_avg', latency_penalty_avg,
    'latency_penalty_p95', latency_penalty_p95,
    'confidence_avg', confidence_avg,
    'threshold_warn', 0.25,
    'threshold_critical', 0.50
  ) as details
from recent_hour
where event_count >= 10
  and latency_penalty_p95 >= 0.25

union all

select
  tenant_id,
  app_id,
  policy_version,
  null::text as precision_level,
  'fuel.error.pressure'::text as alert_code,
  case
    when error_rate >= 0.08 or timeout_rate >= 0.05 then 'critical'
    else 'warn'
  end as severity,
  latest_at as detected_at,
  format('Error/fallback pressure above threshold for %s/%s (%s)', tenant_id, app_id, policy_version) as summary,
  jsonb_build_object(
    'window', '60m',
    'event_count', event_count,
    'error_rate', error_rate,
    'fallback_rate', fallback_rate,
    'timeout_rate', timeout_rate,
    'threshold_error_warn', 0.03,
    'threshold_error_critical', 0.08,
    'threshold_timeout_warn', 0.02,
    'threshold_timeout_critical', 0.05
  ) as details
from recent_hour
where event_count >= 10
  and (error_rate >= 0.03 or fallback_rate >= 0.10 or timeout_rate >= 0.02)

union all

select
  tenant_id,
  app_id,
  policy_version,
  null::text as precision_level,
  'fuel.energy.pressure'::text as alert_code,
  case
    when energy_penalty_total / nullif(fuel_points_total, 0) >= 0.25 then 'critical'
    else 'warn'
  end as severity,
  latest_at as detected_at,
  format('Energy pressure above threshold for %s/%s (%s)', tenant_id, app_id, policy_version) as summary,
  jsonb_build_object(
    'window', '60m',
    'event_count', event_count,
    'energy_penalty_total', energy_penalty_total,
    'fuel_points_total', fuel_points_total,
    'energy_penalty_share', energy_penalty_total / nullif(fuel_points_total, 0),
    'threshold_warn', 0.15,
    'threshold_critical', 0.25
  ) as details
from recent_hour
where event_count >= 10
  and energy_penalty_total / nullif(fuel_points_total, 0) >= 0.15

union all

select
  tenant_id,
  app_id,
  policy_version,
  null::text as precision_level,
  'fuel.drift.high'::text as alert_code,
  case
    when usd_drift_abs_total / nullif(usd_effective_total, 0) >= 0.25 then 'critical'
    else 'warn'
  end as severity,
  latest_at as detected_at,
  format('Settlement drift above threshold for %s/%s (%s)', tenant_id, app_id, policy_version) as summary,
  jsonb_build_object(
    'window', '24h',
    'event_count', event_count,
    'usd_effective_total', usd_effective_total,
    'usd_drift_abs_total', usd_drift_abs_total,
    'drift_ratio_abs', usd_drift_abs_total / nullif(usd_effective_total, 0),
    'threshold_warn', 0.15,
    'threshold_critical', 0.25
  ) as details
from drift_day
where event_count >= 20
  and usd_effective_total > 0
  and usd_drift_abs_total / nullif(usd_effective_total, 0) >= 0.15

union all

select
  tenant_id,
  app_id,
  policy_version,
  null::text as precision_level,
  'fuel.coverage.low_precision'::text as alert_code,
  case
    when l0_l1_count::numeric / nullif(event_count::numeric, 0) >= 0.45 then 'critical'
    else 'warn'
  end as severity,
  latest_at as detected_at,
  format('Precision coverage degraded for %s/%s (%s)', tenant_id, app_id, policy_version) as summary,
  jsonb_build_object(
    'window', '24h',
    'event_count', event_count,
    'l0_l1_ratio', l0_l1_count::numeric / nullif(event_count::numeric, 0),
    'l3_ratio', l3_count::numeric / nullif(event_count::numeric, 0),
    'confidence_avg', confidence_avg,
    'threshold_warn_l0_l1_ratio', 0.25,
    'threshold_critical_l0_l1_ratio', 0.45,
    'threshold_warn_l3_ratio_below', 0.20
  ) as details
from coverage_day
where event_count >= 20
  and (
    l0_l1_count::numeric / nullif(event_count::numeric, 0) >= 0.25
    or l3_count::numeric / nullif(event_count::numeric, 0) < 0.20
  );

create or replace function app.materialize_fuel_ops_jobs(
  p_job_name text default 'baseline_and_alerts',
  p_reference_time timestamptz default now()
)
returns table (
  id bigint,
  job_name text,
  evidence_date date,
  tenant_id text,
  app_id text,
  policy_version text,
  status text,
  metrics jsonb,
  evidence jsonb
)
language plpgsql
security definer
as $$
declare
  v_evidence_date date := timezone('UTC', p_reference_time)::date;
begin
  if p_job_name in ('baseline_snapshot', 'baseline_and_alerts') then
    delete from fuel_ops_job_runs
    where fuel_ops_job_runs.job_name = 'baseline_snapshot'
      and fuel_ops_job_runs.evidence_date = v_evidence_date;

    insert into fuel_ops_job_runs (
      job_name,
      evidence_date,
      tenant_id,
      app_id,
      policy_version,
      status,
      started_at,
      completed_at,
      metrics,
      evidence
    )
    select
      'baseline_snapshot',
      v_evidence_date,
      tenant_id,
      app_id,
      policy_version,
      case
        when sum(case when envelope_status = 'above_p95' then event_count else 0 end) > 0 then 'warn'
        else 'ok'
      end,
      p_reference_time,
      now(),
      jsonb_build_object(
        'window_from', min(window_start),
        'window_to', max(window_start),
        'event_count', sum(event_count),
        'above_p95_count', sum(case when envelope_status = 'above_p95' then event_count else 0 end),
        'above_p75_count', sum(case when envelope_status = 'above_p75' then event_count else 0 end),
        'cold_start_count', sum(case when envelope_status = 'cold_start' then event_count else 0 end),
        'fallback_count', sum(fallback_count),
        'timeout_count', sum(timeout_count)
      ),
      jsonb_build_object(
        'baseline_ready', bool_or(envelope_status <> 'cold_start'),
        'latest_window_start', max(window_start)
      )
    from fuel_window_realtime_v1 fw
    where fw.window_start >= p_reference_time - interval '24 hours'
    group by fw.tenant_id, fw.app_id, fw.policy_version;
  end if;

  if p_job_name in ('alerts_snapshot', 'baseline_and_alerts') then
    delete from fuel_ops_job_runs
    where fuel_ops_job_runs.job_name = 'alerts_snapshot'
      and fuel_ops_job_runs.evidence_date = v_evidence_date;

    insert into fuel_ops_job_runs (
      job_name,
      evidence_date,
      tenant_id,
      app_id,
      policy_version,
      status,
      started_at,
      completed_at,
      metrics,
      evidence
    )
    select
      'alerts_snapshot',
      v_evidence_date,
      tenant_id,
      app_id,
      policy_version,
      case when count(*) filter (where severity = 'critical') > 0 then 'failed'
           when count(*) > 0 then 'warn'
           else 'ok'
      end,
      p_reference_time,
      now(),
      jsonb_build_object(
        'open_alerts', count(*),
        'critical_alerts', count(*) filter (where severity = 'critical'),
        'warn_alerts', count(*) filter (where severity = 'warn')
      ),
      jsonb_build_object(
        'codes', coalesce(jsonb_agg(alert_code order by alert_code), '[]'::jsonb),
        'latest_detected_at', max(detected_at)
      )
    from fuel_alert_candidates_v1 fac
    group by fac.tenant_id, fac.app_id, fac.policy_version;
  end if;

  return query
  select r.id, r.job_name, r.evidence_date, r.tenant_id, r.app_id, r.policy_version, r.status, r.metrics, r.evidence
  from fuel_ops_job_runs r
  where r.evidence_date = v_evidence_date
    and (p_job_name = 'baseline_and_alerts' or r.job_name = p_job_name)
  order by r.job_name, r.tenant_id nulls first, r.app_id nulls first, r.policy_version nulls first;
end;
$$;

grant execute on function app.materialize_fuel_ops_jobs(text, timestamptz) to authenticated;
grant execute on function app.materialize_fuel_ops_jobs(text, timestamptz) to service_role;

commit;
