-- Migration 011: Fuel valuation + control points (Layer B/C)
-- Depends on: 004_onboarding (fuel_events)

begin;

-- ─── Price card registry (versioned) ─────────────────────────────────────────

create table if not exists fuel_price_cards (
  id              bigserial primary key,
  version         text not null,
  unit_type       text not null,
  source_pattern  text not null default '%',
  price_per_unit  numeric not null check (price_per_unit >= 0),
  currency        text not null default 'USD',
  effective_from  timestamptz not null,
  effective_to    timestamptz,
  created_at      timestamptz not null default now(),
  unique (version, unit_type, source_pattern, effective_from)
);

create index if not exists idx_fuel_price_cards_lookup
  on fuel_price_cards (unit_type, effective_from desc);

alter table fuel_price_cards enable row level security;

drop policy if exists fuel_price_cards_select on fuel_price_cards;
create policy fuel_price_cards_select on fuel_price_cards
  for select using (app.current_user_id() is not null);

drop policy if exists fuel_price_cards_mutate_admin on fuel_price_cards;
create policy fuel_price_cards_mutate_admin on fuel_price_cards
  for all using (app.current_user_id() = 'founder')
  with check (app.current_user_id() = 'founder');

insert into fuel_price_cards (
  version, unit_type, source_pattern, price_per_unit, currency, effective_from
) values
  ('2026-03', 'llm_tokens', 'openai:%', 0.00003, 'USD', '2026-03-01T00:00:00Z'),
  ('2026-03', 'llm_tokens', 'anthropic:%', 0.000024, 'USD', '2026-03-01T00:00:00Z'),
  ('2026-03', 'llm_tokens', 'ollama:%', 0.0, 'USD', '2026-03-01T00:00:00Z'),
  ('2026-03', 'api_call', '%', 0.0, 'USD', '2026-03-01T00:00:00Z'),
  ('2026-03', 'code_event', '%', 0.0, 'USD', '2026-03-01T00:00:00Z')
on conflict do nothing;

-- ─── Layer B: valuation ledger (upsert-friendly) ─────────────────────────────

create table if not exists fuel_valuations (
  event_id           text primary key references fuel_events(event_id) on delete cascade,
  usd_estimated      numeric,
  usd_settled        numeric,
  energy_kwh         numeric,
  carbon_gco2e       numeric,
  price_card_version text not null default '2026-03',
  valuation_version  text not null default 'fuel-valuation.v1',
  valuation_source   text not null
                     check (valuation_source in ('price_card', 'provider_usage_api', 'provider_cost_api', 'metered')),
  precision_level    text not null
                     check (precision_level in ('L0', 'L1', 'L2', 'L3')),
  confidence         numeric not null default 0.0 check (confidence >= 0 and confidence <= 1),
  metadata           jsonb not null default '{}'::jsonb,
  created_at         timestamptz not null default now(),
  updated_at         timestamptz not null default now()
);

create index if not exists idx_fuel_valuations_precision
  on fuel_valuations (precision_level, updated_at desc);

create index if not exists idx_fuel_valuations_source
  on fuel_valuations (valuation_source, updated_at desc);

create or replace function app.update_fuel_valuation_timestamp()
returns trigger
language plpgsql
as $$
begin
  new.updated_at = now();
  return new;
end;
$$;

drop trigger if exists trg_fuel_valuations_updated on fuel_valuations;
create trigger trg_fuel_valuations_updated
  before update on fuel_valuations
  for each row execute function app.update_fuel_valuation_timestamp();

alter table fuel_valuations enable row level security;

drop policy if exists fuel_valuations_select on fuel_valuations;
create policy fuel_valuations_select on fuel_valuations
  for select using (
    exists (
      select 1
      from fuel_events fe
      where fe.event_id = fuel_valuations.event_id
        and app.is_app_member(fe.tenant_id, fe.app_id)
    )
  );

drop policy if exists fuel_valuations_insert on fuel_valuations;
create policy fuel_valuations_insert on fuel_valuations
  for insert with check (
    exists (
      select 1
      from fuel_events fe
      where fe.event_id = fuel_valuations.event_id
        and app.is_app_member(fe.tenant_id, fe.app_id)
    )
  );

drop policy if exists fuel_valuations_update on fuel_valuations;
create policy fuel_valuations_update on fuel_valuations
  for update using (
    exists (
      select 1
      from fuel_events fe
      where fe.event_id = fuel_valuations.event_id
        and app.is_app_member(fe.tenant_id, fe.app_id)
    )
  )
  with check (
    exists (
      select 1
      from fuel_events fe
      where fe.event_id = fuel_valuations.event_id
        and app.is_app_member(fe.tenant_id, fe.app_id)
    )
  );

-- ─── Layer C policy (versioned) ──────────────────────────────────────────────

create table if not exists fuel_control_policy_versions (
  policy_version text primary key,
  slo_ttft_ms    integer not null,
  slo_latency_ms integer not null,
  k_latency      numeric not null,
  k_errors       numeric not null,
  k_energy       numeric not null,
  updated_at     timestamptz not null default now()
);

alter table fuel_control_policy_versions enable row level security;

drop policy if exists fuel_control_policy_versions_select on fuel_control_policy_versions;
create policy fuel_control_policy_versions_select on fuel_control_policy_versions
  for select using (app.current_user_id() is not null);

drop policy if exists fuel_control_policy_versions_mutate_admin on fuel_control_policy_versions;
create policy fuel_control_policy_versions_mutate_admin on fuel_control_policy_versions
  for all using (app.current_user_id() = 'founder')
  with check (app.current_user_id() = 'founder');

insert into fuel_control_policy_versions (
  policy_version, slo_ttft_ms, slo_latency_ms, k_latency, k_errors, k_energy
) values (
  'fuel-policy.v1', 1200, 1500, 0.50, 1.00, 0.25
)
on conflict (policy_version) do nothing;

-- ─── Helper parse function for JSON metadata numbers ─────────────────────────

create or replace function app.to_numeric_or_zero(raw text)
returns numeric
language sql
immutable
as $$
  select case
    when raw is null then 0
    when raw ~ '^-?[0-9]+(\.[0-9]+)?$' then raw::numeric
    else 0
  end
$$;

-- ─── Layer C: derived points view ────────────────────────────────────────────

create or replace view fuel_points_v1 as
with latest_policy as (
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
    coalesce(fv.usd_settled, fv.usd_estimated, 0) as usd_effective,
    coalesce(fv.usd_estimated, 0) as usd_estimated,
    fv.usd_settled,
    coalesce(fv.energy_kwh, 0) as energy_kwh,
    coalesce(fv.carbon_gco2e, 0) as carbon_gco2e,
    coalesce(fv.price_card_version, 'unpriced') as price_card_version,
    coalesce(fv.valuation_version, 'fuel-valuation.v1') as valuation_version,
    coalesce(fv.precision_level, 'L0') as precision_level,
    coalesce(fv.confidence, 0) as confidence
  from fuel_events fe
  left join fuel_valuations fv on fv.event_id = fe.event_id
),
scored as (
  select
    e.*,
    p.policy_version,
    p.slo_ttft_ms,
    p.slo_latency_ms,
    p.k_latency,
    p.k_errors,
    p.k_energy,
    (e.usd_effective * 1000.0) as base_cost_points,
    app.to_numeric_or_zero(e.metadata->>'latency_ms') as latency_ms,
    app.to_numeric_or_zero(e.metadata->>'ttft_ms') as ttft_ms,
    app.to_numeric_or_zero(e.metadata->>'retry_count') as retry_count,
    case
      when lower(coalesce(e.metadata->>'fallback_used', 'false')) in ('true', '1', 'yes') then 1
      else 0
    end as fallback_used
  from enriched e
  cross join lateral (
    select
      coalesce(lp.policy_version, 'fuel-policy.v1') as policy_version,
      coalesce(lp.slo_ttft_ms, 1200) as slo_ttft_ms,
      coalesce(lp.slo_latency_ms, 1500) as slo_latency_ms,
      coalesce(lp.k_latency, 0.50)::numeric as k_latency,
      coalesce(lp.k_errors, 1.00)::numeric as k_errors,
      coalesce(lp.k_energy, 0.25)::numeric as k_energy
    from latest_policy lp
  ) p
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
  base_cost_points,
  greatest(
    0,
    (
      greatest(latency_ms, ttft_ms)
      - greatest(slo_latency_ms::numeric, slo_ttft_ms::numeric)
    ) / nullif(greatest(slo_latency_ms::numeric, slo_ttft_ms::numeric), 0)
  ) * k_latency as penalty_latency,
  (
    case when outcome <> 'ok' then k_errors else 0 end
    + retry_count * 0.20
    + fallback_used * 0.50
  ) as penalty_errors,
  (energy_kwh * k_energy) as penalty_energy,
  (
    base_cost_points
    * (
      1
      + (
        greatest(
          0,
          (
            greatest(latency_ms, ttft_ms)
            - greatest(slo_latency_ms::numeric, slo_ttft_ms::numeric)
          ) / nullif(greatest(slo_latency_ms::numeric, slo_ttft_ms::numeric), 0)
        ) * k_latency
      )
      + (
        case when outcome <> 'ok' then k_errors else 0 end
        + retry_count * 0.20
        + fallback_used * 0.50
      )
      + (energy_kwh * k_energy)
    )
  ) as fuel_points_total,
  now() as computed_at
from scored;

commit;
