-- Migration 022: fix fuel ops materialize function for applied environments
-- Depends on: 019_fuel_policy_alerts_ops

begin;

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
set search_path = app, public
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
