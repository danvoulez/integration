-- Migration 021: Supabase security advisor + realtime hardening
-- Depends on: 019_fuel_policy_alerts_ops, 020_edge_control_idempotency

begin;

-- SECURITY DEFINER functions should always pin the search_path.
alter function app.provision_user_membership(text, text, text, text)
  set search_path = app, public;

alter function app.materialize_fuel_ops_jobs(text, timestamptz)
  set search_path = app, public;

-- Views exposed through PostgREST should evaluate with caller permissions.
alter view public.v_active_service_tokens
  set (security_invoker = true);

alter view public.fuel_points_v1
  set (security_invoker = true);

alter view public.fuel_valuation_drift_v1
  set (security_invoker = true);

alter view public.fuel_window_metrics_v1
  set (security_invoker = true);

alter view public.fuel_window_baseline_v1
  set (security_invoker = true);

alter view public.fuel_window_realtime_v1
  set (security_invoker = true);

alter view public.fuel_policy_precision_daily_v1
  set (security_invoker = true);

alter view public.fuel_llm_mode_provider_metrics_v1
  set (security_invoker = true);

alter view public.fuel_policy_calibration_daily_v1
  set (security_invoker = true);

alter view public.fuel_alert_candidates_v1
  set (security_invoker = true);

-- Enable CDC-based Realtime on the operational tables that matter for the stack.
do $$
begin
  if exists (select 1 from pg_publication where pubname = 'supabase_realtime') then
    begin
      alter publication supabase_realtime add table public.code247_jobs;
    exception
      when duplicate_object then null;
    end;

    begin
      alter publication supabase_realtime add table public.code247_checkpoints;
    exception
      when duplicate_object then null;
    end;

    begin
      alter publication supabase_realtime add table public.code247_events;
    exception
      when duplicate_object then null;
    end;

    begin
      alter publication supabase_realtime add table public.fuel_events;
    exception
      when duplicate_object then null;
    end;

    begin
      alter publication supabase_realtime add table public.fuel_ops_job_runs;
    exception
      when duplicate_object then null;
    end;
  end if;
end
$$;

commit;
