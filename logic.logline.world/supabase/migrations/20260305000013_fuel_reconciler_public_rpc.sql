-- Migration 013: Public RPC wrapper for fuel L0 reconciler
-- Depends on: 012_fuel_l0_reconciler

begin;

create or replace function public.backfill_fuel_valuations_l0(
  p_from timestamptz default now() - interval '7 days',
  p_to timestamptz default now(),
  p_price_card_version text default null
)
returns integer
language sql
as $$
  select app.backfill_fuel_valuations_l0(p_from, p_to, p_price_card_version)
$$;

grant execute on function public.backfill_fuel_valuations_l0(timestamptz, timestamptz, text)
to authenticated, service_role;

commit;
