-- Migration 016: Fuel window metrics + seasonal baseline envelope
-- Depends on: 011_fuel_valuation_points, 012_fuel_l0_reconciler

begin;

-- Windowed metrics (5-minute buckets) used by realtime + statistics dashboards.
create or replace view fuel_window_metrics_v1 as
with base as (
  select
    to_timestamp(floor(extract(epoch from fe.occurred_at) / 300) * 300)::timestamptz as window_start,
    fe.tenant_id,
    fe.app_id,
    fe.source,
    count(*)::bigint as event_count,
    sum(coalesce(fp.fuel_points_total, 0))::numeric as fuel_points_total,
    sum(coalesce(fp.usd_effective, 0))::numeric as usd_effective_total,
    sum(coalesce(fp.usd_estimated, 0))::numeric as usd_estimated_total,
    sum(coalesce(fp.usd_settled, 0))::numeric as usd_settled_total,
    sum(case when coalesce(fp.outcome, 'ok') <> 'ok' then 1 else 0 end)::bigint as error_count,
    percentile_cont(0.95) within group (order by app.to_numeric_or_zero(fe.metadata->>'latency_ms'))
      filter (where fe.metadata ? 'latency_ms')::numeric as latency_p95_ms,
    percentile_cont(0.95) within group (order by app.to_numeric_or_zero(fe.metadata->>'ttft_ms'))
      filter (where fe.metadata ? 'ttft_ms')::numeric as ttft_p95_ms
  from fuel_events fe
  left join fuel_points_v1 fp on fp.event_id = fe.event_id
  group by 1, 2, 3, 4
)
select
  window_start,
  tenant_id,
  app_id,
  source,
  event_count,
  fuel_points_total,
  usd_effective_total,
  usd_estimated_total,
  usd_settled_total,
  error_count,
  case when event_count > 0 then error_count::numeric / event_count::numeric else 0 end as error_rate,
  latency_p95_ms,
  ttft_p95_ms,
  extract(isodow from timezone('UTC', window_start))::int as iso_dow,
  extract(hour from timezone('UTC', window_start))::int as hour_utc,
  floor(extract(minute from timezone('UTC', window_start)) / 5)::int as minute_bucket_5,
  now() as computed_at
from base;

-- Seasonal baseline envelope by weekday/hour/5-min slot.
create or replace view fuel_window_baseline_v1 as
select
  tenant_id,
  app_id,
  source,
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
  percentile_cont(0.95) within group (order by coalesce(latency_p95_ms, 0))::numeric as latency_p95_envelope,
  percentile_cont(0.95) within group (order by coalesce(ttft_p95_ms, 0))::numeric as ttft_p95_envelope,
  min(window_start) as baseline_window_from,
  max(window_start) as baseline_window_to,
  now() as computed_at
from fuel_window_metrics_v1
where window_start >= now() - interval '60 days'
  and window_start < now() - interval '5 minutes'
group by 1, 2, 3, 4, 5, 6
having count(*) >= 8;

-- Realtime view joining current buckets with baseline envelope.
create or replace view fuel_window_realtime_v1 as
select
  wm.window_start,
  wm.tenant_id,
  wm.app_id,
  wm.source,
  wm.event_count,
  wm.fuel_points_total,
  wm.usd_effective_total,
  wm.usd_estimated_total,
  wm.usd_settled_total,
  wm.error_count,
  wm.error_rate,
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
 and wb.iso_dow = wm.iso_dow
 and wb.hour_utc = wm.hour_utc
 and wb.minute_bucket_5 = wm.minute_bucket_5;

commit;
