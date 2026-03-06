-- Migration 015: Fuel metadata guardrails (fail-closed on inserts)
-- Depends on: 004_onboarding (fuel_events)

begin;

-- Ensure metadata is always present for new rows.
update fuel_events
set metadata = '{}'::jsonb
where metadata is null;

alter table fuel_events
  alter column metadata set default '{}'::jsonb;

alter table fuel_events
  alter column metadata set not null;

-- Core causal metadata required on every fuel event.
alter table fuel_events
  add constraint fuel_events_metadata_object_chk
  check (jsonb_typeof(metadata) = 'object')
  not valid;

alter table fuel_events
  add constraint fuel_events_metadata_core_chk
  check (
    (metadata ? 'event_type')
    and jsonb_typeof(metadata->'event_type') = 'string'
    and length(btrim(metadata->>'event_type')) > 0
    and (metadata ? 'trace_id')
    and jsonb_typeof(metadata->'trace_id') = 'string'
    and length(btrim(metadata->>'trace_id')) > 0
    and (metadata ? 'parent_event_id')
    and (
      metadata->'parent_event_id' = 'null'::jsonb
      or jsonb_typeof(metadata->'parent_event_id') = 'string'
    )
    and (metadata ? 'outcome')
    and jsonb_typeof(metadata->'outcome') = 'string'
    and length(btrim(metadata->>'outcome')) > 0
  )
  not valid;

-- For LLM-token fuel, enforce richer dimensions needed by valuation/analysis.
alter table fuel_events
  add constraint fuel_events_metadata_llm_chk
  check (
    unit_type <> 'llm_tokens'
    or (
      (metadata ? 'provider')
      and jsonb_typeof(metadata->'provider') = 'string'
      and length(btrim(metadata->>'provider')) > 0
      and (metadata ? 'model')
      and jsonb_typeof(metadata->'model') = 'string'
      and length(btrim(metadata->>'model')) > 0
      and (metadata ? 'prompt_tokens')
      and jsonb_typeof(metadata->'prompt_tokens') = 'number'
      and (metadata ? 'completion_tokens')
      and jsonb_typeof(metadata->'completion_tokens') = 'number'
      and (metadata ? 'latency_ms')
      and jsonb_typeof(metadata->'latency_ms') = 'number'
    )
  )
  not valid;

commit;
