-- Migration 012: Fuel L0 valuation reconciler + drift view
-- Depends on: 011_fuel_valuation_points

begin;

-- Resolve price card rule for a fuel event.
create or replace function app.match_fuel_price(
  p_unit_type text,
  p_source text,
  p_occurred_at timestamptz,
  p_version text default null
)
returns table (
  version text,
  price_per_unit numeric,
  currency text
)
language sql
stable
as $$
  select
    pc.version,
    pc.price_per_unit,
    pc.currency
  from fuel_price_cards pc
  where pc.unit_type = p_unit_type
    and (p_version is null or pc.version = p_version)
    and p_source like pc.source_pattern
    and pc.effective_from <= p_occurred_at
    and (pc.effective_to is null or p_occurred_at < pc.effective_to)
  order by
    char_length(pc.source_pattern) desc,
    pc.effective_from desc
  limit 1
$$;

-- Backfill/update L0 valuations from price cards for a time window.
create or replace function app.backfill_fuel_valuations_l0(
  p_from timestamptz default now() - interval '7 days',
  p_to timestamptz default now(),
  p_price_card_version text default null
)
returns integer
language plpgsql
as $$
declare
  v_affected integer := 0;
begin
  with priced as (
    select
      fe.event_id,
      fe.units,
      fe.unit_type,
      fe.source,
      fe.occurred_at,
      mp.version as price_card_version,
      mp.price_per_unit,
      mp.currency
    from fuel_events fe
    join lateral app.match_fuel_price(
      fe.unit_type,
      fe.source,
      fe.occurred_at,
      p_price_card_version
    ) mp on true
    where fe.occurred_at >= p_from
      and fe.occurred_at < p_to
  )
  insert into fuel_valuations (
    event_id,
    usd_estimated,
    usd_settled,
    energy_kwh,
    carbon_gco2e,
    price_card_version,
    valuation_version,
    valuation_source,
    precision_level,
    confidence,
    metadata
  )
  select
    p.event_id,
    (p.units * p.price_per_unit) as usd_estimated,
    null::numeric as usd_settled,
    null::numeric as energy_kwh,
    null::numeric as carbon_gco2e,
    p.price_card_version,
    'fuel-valuation.v1' as valuation_version,
    'price_card' as valuation_source,
    'L0' as precision_level,
    case
      when p.unit_type = 'llm_tokens' then 0.85
      when p.unit_type = 'code_event' then 0.75
      else 0.70
    end as confidence,
    jsonb_build_object(
      'currency', p.currency,
      'source', p.source,
      'unit_type', p.unit_type,
      'method', 'units * price_card'
    ) as metadata
  from priced p
  on conflict (event_id) do update
    set
      usd_estimated = excluded.usd_estimated,
      price_card_version = excluded.price_card_version,
      valuation_version = excluded.valuation_version,
      valuation_source = excluded.valuation_source,
      precision_level = case
        when fuel_valuations.usd_settled is not null then fuel_valuations.precision_level
        else excluded.precision_level
      end,
      confidence = case
        when fuel_valuations.usd_settled is not null then fuel_valuations.confidence
        else excluded.confidence
      end,
      metadata = fuel_valuations.metadata || excluded.metadata,
      updated_at = now()
    where fuel_valuations.usd_settled is null;

  get diagnostics v_affected = row_count;
  return coalesce(v_affected, 0);
end;
$$;

-- Daily drift summary for observability.
create or replace view fuel_valuation_drift_v1 as
select
  date_trunc('day', fe.occurred_at) as day,
  fe.tenant_id,
  fe.app_id,
  fe.source,
  count(*) as event_count,
  sum(coalesce(fv.usd_estimated, 0)) as usd_estimated_total,
  sum(coalesce(fv.usd_settled, 0)) as usd_settled_total,
  sum(coalesce(fv.usd_settled, fv.usd_estimated, 0)) as usd_effective_total,
  sum(coalesce(fv.usd_settled, 0) - coalesce(fv.usd_estimated, 0)) as usd_drift_total
from fuel_events fe
left join fuel_valuations fv on fv.event_id = fe.event_id
group by 1, 2, 3, 4;

commit;
