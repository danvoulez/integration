import { sql } from './index';

let initPromise: Promise<void> | null = null;

export async function ensureDbSchema(): Promise<void> {
  if (!initPromise) {
    initPromise = (async () => {
      // ─── Identity / RBAC tables ─────────────────────────────────────────────

      await sql`
        create table if not exists users (
          user_id      text primary key,
          email        text,
          display_name text,
          created_at   timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists tenants (
          tenant_id  text primary key,
          slug       text unique not null,
          name       text not null,
          created_at timestamptz not null default now()
        );
      `;
      // Add slug column if upgrading from old schema
      await sql`alter table tenants add column if not exists slug text;`;
      // Backfill slug from tenant_id for pre-existing rows
      await sql`update tenants set slug = tenant_id where slug is null;`;

      await sql`
        create table if not exists apps (
          app_id     text primary key,
          tenant_id  text not null references tenants(tenant_id) on delete cascade,
          name       text not null,
          created_at timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists tenant_memberships (
          tenant_id  text not null references tenants(tenant_id) on delete cascade,
          user_id    text not null references users(user_id) on delete cascade,
          role       text not null default 'member',
          created_at timestamptz not null default now(),
          primary key (tenant_id, user_id)
        );
      `;

      await sql`
        create table if not exists app_memberships (
          app_id     text not null references apps(app_id) on delete cascade,
          tenant_id  text not null references tenants(tenant_id) on delete cascade,
          user_id    text not null references users(user_id) on delete cascade,
          role       text not null default 'member',
          created_at timestamptz not null default now(),
          primary key (app_id, tenant_id, user_id)
        );
      `;

      await sql`
        create table if not exists user_capabilities (
          user_id    text not null references users(user_id) on delete cascade,
          capability text not null,
          granted_by text references users(user_id),
          granted_at timestamptz not null default now(),
          primary key (user_id, capability)
        );
      `;

      await sql`
        create table if not exists tenant_email_allowlist (
          tenant_id        text not null references tenants(tenant_id) on delete cascade,
          email_normalized text not null,
          role_default     text not null default 'member',
          app_defaults     jsonb not null default '[]',
          expires_at       timestamptz,
          created_at       timestamptz not null default now(),
          primary key (tenant_id, email_normalized)
        );
      `;

      await sql`
        create table if not exists user_provider_keys (
          key_id        text primary key default gen_random_uuid()::text,
          tenant_id     text not null references tenants(tenant_id) on delete cascade,
          app_id        text not null references apps(app_id) on delete cascade,
          user_id       text not null references users(user_id) on delete cascade,
          provider      text not null,
          key_label     text not null,
          encrypted_key text not null,
          metadata      jsonb not null default '{}',
          created_at    timestamptz not null default now(),
          updated_at    timestamptz not null default now(),
          unique (tenant_id, app_id, user_id, provider, key_label)
        );
      `;

      await sql`
        create table if not exists cli_auth_challenges (
          challenge_id  text primary key default gen_random_uuid()::text,
          nonce         text not null unique,
          status        text not null default 'pending',
          device_name   text,
          user_id       text references users(user_id),
          tenant_id     text references tenants(tenant_id),
          session_token text,
          expires_at    timestamptz not null default (now() + interval '5 minutes'),
          approved_at   timestamptz,
          created_at    timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists founder_signing_keys (
          key_id      text primary key default gen_random_uuid()::text,
          user_id     text not null references users(user_id) on delete cascade,
          public_key  text not null,
          algorithm   text not null default 'ed25519',
          status      text not null default 'active',
          created_at  timestamptz not null default now(),
          revoked_at  timestamptz
        );
      `;

      await sql`
        create table if not exists protected_intents (
          intent_id           text primary key default gen_random_uuid()::text,
          actor_user_id       text not null references users(user_id),
          tenant_id           text references tenants(tenant_id),
          app_id              text references apps(app_id),
          nonce               text not null unique,
          payload_hash        text not null,
          signing_key_id      text not null references founder_signing_keys(key_id),
          signature           text not null,
          expires_at          timestamptz not null,
          verification_status text not null default 'pending',
          verified_at         timestamptz,
          created_at          timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists protected_action_audit (
          id               bigserial primary key,
          actor_user_id    text not null,
          intent_id        text references protected_intents(intent_id),
          action_type      text not null,
          payload_summary  text,
          decision         text not null,
          deny_reason      text,
          execution_result text,
          device_info      jsonb,
          recorded_at      timestamptz not null default now()
        );
      `;

      // ─── Panel tables ────────────────────────────────────────────────────────

      await sql`
        create table if not exists panels (
          panel_id     text primary key,
          workspace_id text not null default 'default',
          app_id       text not null default 'ublx',
          name         text not null,
          position     integer not null default 0,
          version      text not null default '1.0.0',
          created_at   timestamptz not null default now(),
          updated_at   timestamptz not null default now()
        );
      `;

      await sql`alter table panels add column if not exists workspace_id text not null default 'default';`;
      await sql`alter table panels add column if not exists app_id text not null default 'ublx';`;

      await sql`
        create table if not exists panel_components (
          instance_id  text primary key,
          panel_id     text not null references panels(panel_id) on delete cascade,
          component_id text not null,
          version      text not null default '1.0.0',
          rect_x       integer not null default 0,
          rect_y       integer not null default 0,
          rect_w       integer not null default 8,
          rect_h       integer not null default 8,
          front_props  text not null default '{}',
          position     integer not null default 0,
          created_at   timestamptz not null default now(),
          updated_at   timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists instance_configs (
          instance_id        text primary key references panel_components(instance_id) on delete cascade,
          source_hub         text,
          source_origin      text,
          source_auth_ref    text,
          source_mode        text,
          source_interval_ms integer,
          proc_executor      text,
          proc_command       text,
          proc_args          text default '[]',
          proc_timeout_ms    integer,
          proc_retries       integer,
          proc_backoff       text,
          proc_error_mode    text,
          updated_at         timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists installed_components (
          component_id text primary key,
          installed_at timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists tab_meta (
          panel_id text primary key references panels(panel_id) on delete cascade,
          icon     text,
          label    text,
          shortcut integer
        );
      `;

      await sql`
        create table if not exists panel_settings (
          panel_id   text primary key references panels(panel_id) on delete cascade,
          settings   text not null default '{}',
          updated_at timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists chat_messages (
          id           text primary key,
          workspace_id text not null default 'default',
          app_id       text not null default 'ublx',
          session_id   text not null,
          panel_id     text,
          instance_id  text,
          role         text not null,
          content      text not null,
          model_used   text,
          latency_ms   integer,
          created_at   timestamptz not null default now()
        );
      `;
      await sql`alter table chat_messages add column if not exists workspace_id text not null default 'default';`;
      await sql`alter table chat_messages add column if not exists app_id text not null default 'ublx';`;

      await sql`
        create table if not exists app_settings (
          key        text primary key,
          value      text not null,
          updated_at timestamptz not null default now()
        );
      `;

      await sql`
        create table if not exists service_status_log (
          id           serial primary key,
          workspace_id text not null default 'default',
          app_id       text not null default 'ublx',
          service_name text not null,
          status       text not null,
          latency_ms   integer,
          recorded_at  timestamptz not null default now()
        );
      `;
      await sql`alter table service_status_log add column if not exists workspace_id text not null default 'default';`;
      await sql`alter table service_status_log add column if not exists app_id text not null default 'ublx';`;

      await sql`
        create table if not exists obs_events (
          event_id     text primary key,
          event_type   text not null,
          occurred_at  timestamptz not null,
          source       text not null,
          request_id   text not null,
          trace_id     text,
          parent_event_id text,
          intention_id text,
          run_id       text,
          issue_id     text,
          pr_id        text,
          deploy_id    text,
          payload      jsonb not null default '{}',
          ingested_at  timestamptz not null default now()
        );
      `;
      await sql`alter table obs_events add column if not exists trace_id text;`;
      await sql`alter table obs_events add column if not exists parent_event_id text;`;

      await sql`
        create table if not exists obs_run_state (
          run_id               text primary key,
          current_event_id     text not null,
          current_event_type   text not null,
          current_occurred_at  timestamptz not null,
          current_source       text not null,
          current_request_id   text not null,
          current_trace_id     text,
          current_parent_event_id text,
          current_intention_id text,
          current_issue_id     text,
          current_pr_id        text,
          current_deploy_id    text,
          current_payload      jsonb not null default '{}',
          last_ingested_at     timestamptz not null default now(),
          updated_at           timestamptz not null default now()
        );
      `;
      await sql`alter table obs_run_state add column if not exists current_trace_id text;`;
      await sql`alter table obs_run_state add column if not exists current_parent_event_id text;`;
      await sql`
        create table if not exists obs_alerts (
          alert_id        text primary key,
          code            text not null,
          severity        text not null,
          status          text not null default 'open',
          summary         text not null,
          details         jsonb not null default '{}',
          source          text,
          intention_id    text,
          run_id          text,
          issue_id        text,
          first_seen_at   timestamptz not null default now(),
          last_seen_at    timestamptz not null default now(),
          acked_at        timestamptz,
          acked_by        text,
          ack_reason      text,
          resolved_at     timestamptz,
          created_at      timestamptz not null default now(),
          updated_at      timestamptz not null default now()
        );
      `;
      await sql`alter table obs_alerts add column if not exists code text;`;
      await sql`alter table obs_alerts add column if not exists severity text;`;
      await sql`alter table obs_alerts add column if not exists status text not null default 'open';`;
      await sql`alter table obs_alerts add column if not exists summary text;`;
      await sql`alter table obs_alerts add column if not exists details jsonb not null default '{}';`;
      await sql`alter table obs_alerts add column if not exists source text;`;
      await sql`alter table obs_alerts add column if not exists intention_id text;`;
      await sql`alter table obs_alerts add column if not exists run_id text;`;
      await sql`alter table obs_alerts add column if not exists issue_id text;`;
      await sql`alter table obs_alerts add column if not exists first_seen_at timestamptz not null default now();`;
      await sql`alter table obs_alerts add column if not exists last_seen_at timestamptz not null default now();`;
      await sql`alter table obs_alerts add column if not exists acked_at timestamptz;`;
      await sql`alter table obs_alerts add column if not exists acked_by text;`;
      await sql`alter table obs_alerts add column if not exists ack_reason text;`;
      await sql`alter table obs_alerts add column if not exists resolved_at timestamptz;`;
      await sql`alter table obs_alerts add column if not exists created_at timestamptz not null default now();`;
      await sql`alter table obs_alerts add column if not exists updated_at timestamptz not null default now();`;

      // ─── Indexes ─────────────────────────────────────────────────────────────

      await sql`create index if not exists idx_panels_workspace_app_position on panels (workspace_id, app_id, position);`;
      await sql`create index if not exists idx_chat_workspace_app_session_created on chat_messages (workspace_id, app_id, session_id, created_at);`;
      await sql`create index if not exists idx_status_workspace_app_recorded on service_status_log (workspace_id, app_id, recorded_at desc);`;
      await sql`create index if not exists idx_obs_events_intention_occurred on obs_events (intention_id, occurred_at desc);`;
      await sql`create index if not exists idx_obs_events_run_occurred on obs_events (run_id, occurred_at desc);`;
      await sql`create index if not exists idx_obs_events_request_id on obs_events (request_id);`;
      await sql`create index if not exists idx_obs_events_trace_id on obs_events (trace_id);`;
      await sql`create index if not exists idx_obs_events_parent_event_id on obs_events (parent_event_id);`;
      await sql`create index if not exists idx_obs_events_event_type_occurred on obs_events (event_type, occurred_at desc);`;
      await sql`create index if not exists idx_obs_run_state_occurred on obs_run_state (current_occurred_at desc);`;
      await sql`create index if not exists idx_obs_run_state_intention on obs_run_state (current_intention_id);`;
      await sql`create index if not exists idx_obs_alerts_status_last_seen on obs_alerts (status, last_seen_at desc);`;
      await sql`create index if not exists idx_obs_alerts_code_status on obs_alerts (code, status);`;
      await sql`create index if not exists idx_obs_alerts_run_status on obs_alerts (run_id, status);`;
      await sql`create index if not exists idx_tenant_memberships_user on tenant_memberships (user_id);`;
      await sql`create index if not exists idx_app_memberships_user on app_memberships (user_id);`;
      await sql`create index if not exists idx_tenants_slug on tenants (slug);`;
      await sql`create index if not exists idx_user_provider_keys_user on user_provider_keys (user_id);`;
      await sql`create index if not exists idx_cli_challenges_status on cli_auth_challenges (status, expires_at);`;
      await sql`create index if not exists idx_founder_keys_user on founder_signing_keys (user_id, status);`;
    })();
  }

  await initPromise;
}
