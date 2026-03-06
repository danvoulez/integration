-- Migration 014: make public fuel reconciler RPC runnable via PostgREST
-- Depends on: 013_fuel_reconciler_public_rpc

begin;

create or replace function public.backfill_fuel_valuations_l0(
  p_from timestamptz default now() - interval '7 days',
  p_to timestamptz default now(),
  p_price_card_version text default null
)
returns integer
language plpgsql
security definer
set search_path = public, app
as $$
begin
  return app.backfill_fuel_valuations_l0(p_from, p_to, p_price_card_version);
end;
$$;

grant execute on function public.backfill_fuel_valuations_l0(timestamptz, timestamptz, text)
to authenticated, service_role;

commit;
