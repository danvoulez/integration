-- Migration 018: Cloud settlement + local energy upsert RPCs (idempotent)
-- Depends on: 011_fuel_valuation_points, 016_fuel_window_baseline

begin;

create or replace function app.apply_fuel_settlement_rows(
  p_rows jsonb,
  p_run_id text default null
)
returns integer
language plpgsql
as $$
declare
  v_affected integer := 0;
begin
  with rows as (
    select
      trim(r.event_id) as event_id,
      greatest(0::numeric, coalesce(r.usd_settled, 0)::numeric) as usd_settled,
      least(1::numeric, greatest(0::numeric, coalesce(r.confidence, 0.90)::numeric)) as confidence,
      coalesce(nullif(r.precision_level, ''), 'L2') as precision_level,
      coalesce(nullif(r.valuation_source, ''), 'provider_cost_api') as valuation_source,
      coalesce(r.metadata, '{}'::jsonb) as metadata
    from jsonb_to_recordset(coalesce(p_rows, '[]'::jsonb)) as r(
      event_id text,
      usd_settled numeric,
      confidence numeric,
      precision_level text,
      valuation_source text,
      metadata jsonb
    )
    where coalesce(trim(r.event_id), '') <> ''
  )
  insert into fuel_valuations (
    event_id,
    usd_settled,
    valuation_source,
    precision_level,
    confidence,
    valuation_version,
    metadata
  )
  select
    rows.event_id,
    rows.usd_settled,
    rows.valuation_source,
    case
      when rows.precision_level in ('L0', 'L1', 'L2', 'L3') then rows.precision_level
      else 'L2'
    end,
    rows.confidence,
    'fuel-valuation.v2',
    rows.metadata
      || case
        when p_run_id is null or btrim(p_run_id) = '' then '{}'::jsonb
        else jsonb_build_object('settlement_run_id', p_run_id)
      end
  from rows
  on conflict (event_id) do update
    set
      usd_settled = excluded.usd_settled,
      valuation_source = excluded.valuation_source,
      precision_level = case
        when fuel_valuations.precision_level = 'L3' then fuel_valuations.precision_level
        else excluded.precision_level
      end,
      confidence = greatest(fuel_valuations.confidence, excluded.confidence),
      valuation_version = 'fuel-valuation.v2',
      metadata = fuel_valuations.metadata
        || excluded.metadata
        || case
          when p_run_id is null or btrim(p_run_id) = '' then '{}'::jsonb
          else jsonb_build_object('settlement_run_id', p_run_id)
        end,
      updated_at = now()
    where fuel_valuations.precision_level <> 'L3'
      and (
        p_run_id is null
        or btrim(p_run_id) = ''
        or coalesce(fuel_valuations.metadata->>'settlement_run_id', '') is distinct from p_run_id
        or fuel_valuations.usd_settled is distinct from excluded.usd_settled
      );

  get diagnostics v_affected = row_count;
  return coalesce(v_affected, 0);
end;
$$;

create or replace function public.apply_fuel_settlement_rows(
  p_rows jsonb,
  p_run_id text default null
)
returns integer
language sql
security definer
set search_path = app, public
as $$
  select app.apply_fuel_settlement_rows(p_rows, p_run_id);
$$;

grant execute on function public.apply_fuel_settlement_rows(jsonb, text)
  to authenticated, service_role;

create or replace function app.upsert_local_energy_measurement(
  p_event_id text,
  p_energy_kwh numeric,
  p_carbon_gco2e numeric,
  p_confidence numeric default 0.72,
  p_metadata jsonb default '{}'::jsonb
)
returns boolean
language plpgsql
as $$
declare
  v_affected integer := 0;
begin
  if p_event_id is null or btrim(p_event_id) = '' then
    return false;
  end if;

  insert into fuel_valuations (
    event_id,
    energy_kwh,
    carbon_gco2e,
    valuation_source,
    precision_level,
    confidence,
    valuation_version,
    metadata
  )
  values (
    p_event_id,
    greatest(0::numeric, coalesce(p_energy_kwh, 0)),
    greatest(0::numeric, coalesce(p_carbon_gco2e, 0)),
    'metered',
    'L1',
    least(1::numeric, greatest(0::numeric, coalesce(p_confidence, 0.72))),
    'fuel-valuation.v2',
    coalesce(p_metadata, '{}'::jsonb)
      || jsonb_build_object('energy_method', 'local_latency_metered')
  )
  on conflict (event_id) do update
    set
      energy_kwh = excluded.energy_kwh,
      carbon_gco2e = excluded.carbon_gco2e,
      valuation_source = case
        when fuel_valuations.valuation_source in ('provider_usage_api', 'provider_cost_api')
          then fuel_valuations.valuation_source
        else excluded.valuation_source
      end,
      precision_level = case
        when fuel_valuations.precision_level in ('L2', 'L3')
          then fuel_valuations.precision_level
        else excluded.precision_level
      end,
      confidence = greatest(fuel_valuations.confidence, excluded.confidence),
      valuation_version = 'fuel-valuation.v2',
      metadata = fuel_valuations.metadata || excluded.metadata,
      updated_at = now();

  get diagnostics v_affected = row_count;
  return coalesce(v_affected, 0) > 0;
end;
$$;

create or replace function public.upsert_local_energy_measurement(
  p_event_id text,
  p_energy_kwh numeric,
  p_carbon_gco2e numeric,
  p_confidence numeric default 0.72,
  p_metadata jsonb default '{}'::jsonb
)
returns boolean
language sql
security definer
set search_path = app, public
as $$
  select app.upsert_local_energy_measurement(
    p_event_id,
    p_energy_kwh,
    p_carbon_gco2e,
    p_confidence,
    p_metadata
  );
$$;

grant execute on function public.upsert_local_energy_measurement(text, numeric, numeric, numeric, jsonb)
  to authenticated, service_role;

commit;
