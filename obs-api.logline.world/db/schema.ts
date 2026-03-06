import {
  pgTable,
  text,
  integer,
  numeric,
  timestamp,
  serial,
  unique,
  jsonb,
  bigserial,
} from 'drizzle-orm/pg-core';

// ─── 1. panels ───────────────────────────────────────────────────────────────
export const panels = pgTable('panels', {
  panel_id:   text('panel_id').primaryKey(),
  workspace_id: text('workspace_id').notNull().default('default'),
  app_id: text('app_id').notNull().default('ublx'),
  name:       text('name').notNull(),
  position:   integer('position').notNull().default(0),
  version:    text('version').notNull().default('1.0.0'),
  created_at: timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  updated_at: timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 2. panel_components ─────────────────────────────────────────────────────
export const panelComponents = pgTable('panel_components', {
  instance_id:  text('instance_id').primaryKey(),
  panel_id:     text('panel_id')
                  .notNull()
                  .references(() => panels.panel_id, { onDelete: 'cascade' }),
  component_id: text('component_id').notNull(),
  version:      text('version').notNull().default('1.0.0'),
  rect_x:       integer('rect_x').notNull().default(0),
  rect_y:       integer('rect_y').notNull().default(0),
  rect_w:       integer('rect_w').notNull().default(8),
  rect_h:       integer('rect_h').notNull().default(8),
  front_props:  text('front_props').notNull().default('{}'),
  position:     integer('position').notNull().default(0),
  created_at:   timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  updated_at:   timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 3. instance_configs ─────────────────────────────────────────────────────
export const instanceConfigs = pgTable('instance_configs', {
  instance_id:        text('instance_id')
                        .primaryKey()
                        .references(() => panelComponents.instance_id, { onDelete: 'cascade' }),
  source_hub:         text('source_hub'),
  source_origin:      text('source_origin'),
  source_auth_ref:    text('source_auth_ref'),
  source_mode:        text('source_mode'),
  source_interval_ms: integer('source_interval_ms'),
  proc_executor:      text('proc_executor'),
  proc_command:       text('proc_command'),
  proc_args:          text('proc_args').default('[]'),
  proc_timeout_ms:    integer('proc_timeout_ms'),
  proc_retries:       integer('proc_retries'),
  proc_backoff:       text('proc_backoff'),
  proc_error_mode:    text('proc_error_mode'),
  updated_at:         timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 4. installed_components ─────────────────────────────────────────────────
export const installedComponents = pgTable('installed_components', {
  component_id: text('component_id').primaryKey(),
  installed_at: timestamp('installed_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 5. tab_meta ─────────────────────────────────────────────────────────────
export const tabMeta = pgTable('tab_meta', {
  panel_id: text('panel_id')
              .primaryKey()
              .references(() => panels.panel_id, { onDelete: 'cascade' }),
  icon:     text('icon'),
  label:    text('label'),
  shortcut: integer('shortcut'),
});

// ─── 6. panel_settings ───────────────────────────────────────────────────────
export const panelSettings = pgTable('panel_settings', {
  panel_id:    text('panel_id')
                 .primaryKey()
                 .references(() => panels.panel_id, { onDelete: 'cascade' }),
  settings:    text('settings').notNull().default('{}'),
  updated_at:  timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 7. chat_messages ────────────────────────────────────────────────────────
export const chatMessages = pgTable('chat_messages', {
  id:          text('id').primaryKey(),
  workspace_id: text('workspace_id').notNull().default('default'),
  app_id: text('app_id').notNull().default('ublx'),
  session_id:  text('session_id').notNull(),
  panel_id:    text('panel_id'),
  instance_id: text('instance_id'),
  role:        text('role').notNull(),
  content:     text('content').notNull(),
  model_used:  text('model_used'),
  latency_ms:  integer('latency_ms'),
  created_at:  timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 8. app_settings ─────────────────────────────────────────────────────────
export const appSettings = pgTable('app_settings', {
  key:        text('key').primaryKey(),
  value:      text('value').notNull(),
  updated_at: timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 9. service_status_log ───────────────────────────────────────────────────
export const serviceStatusLog = pgTable('service_status_log', {
  id:           serial('id').primaryKey(),
  workspace_id: text('workspace_id').notNull().default('default'),
  app_id: text('app_id').notNull().default('ublx'),
  service_name: text('service_name').notNull(),
  status:       text('status').notNull(),
  latency_ms:   integer('latency_ms'),
  recorded_at:  timestamp('recorded_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 10. Identity tables ─────────────────────────────────────────────────────

export const users = pgTable('users', {
  user_id:      text('user_id').primaryKey(),
  email:        text('email'),
  display_name: text('display_name'),
  created_at:   timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

export const tenants = pgTable('tenants', {
  tenant_id:  text('tenant_id').primaryKey(),
  slug:       text('slug').notNull().unique(),
  name:       text('name').notNull(),
  created_at: timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

export const apps = pgTable('apps', {
  app_id:     text('app_id').primaryKey(),
  tenant_id:  text('tenant_id').notNull().references(() => tenants.tenant_id, { onDelete: 'cascade' }),
  name:       text('name').notNull(),
  created_at: timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 11. Membership tables ───────────────────────────────────────────────────

export const tenantMemberships = pgTable('tenant_memberships', {
  tenant_id:  text('tenant_id').notNull().references(() => tenants.tenant_id, { onDelete: 'cascade' }),
  user_id:    text('user_id').notNull().references(() => users.user_id, { onDelete: 'cascade' }),
  role:       text('role').notNull().default('member'), // member | admin
  created_at: timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
}, (t) => [unique().on(t.tenant_id, t.user_id)]);

export const appMemberships = pgTable('app_memberships', {
  app_id:     text('app_id').notNull().references(() => apps.app_id, { onDelete: 'cascade' }),
  tenant_id:  text('tenant_id').notNull().references(() => tenants.tenant_id, { onDelete: 'cascade' }),
  user_id:    text('user_id').notNull().references(() => users.user_id, { onDelete: 'cascade' }),
  role:       text('role').notNull().default('member'), // member | app_admin
  created_at: timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
}, (t) => [unique().on(t.app_id, t.tenant_id, t.user_id)]);

// ─── 12. Capabilities ────────────────────────────────────────────────────────

export const userCapabilities = pgTable('user_capabilities', {
  user_id:    text('user_id').notNull().references(() => users.user_id, { onDelete: 'cascade' }),
  capability: text('capability').notNull(),
  granted_by: text('granted_by').references(() => users.user_id),
  granted_at: timestamp('granted_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
}, (t) => [unique().on(t.user_id, t.capability)]);

// ─── 13. Email allowlist ──────────────────────────────────────────────────────

export const tenantEmailAllowlist = pgTable('tenant_email_allowlist', {
  tenant_id:        text('tenant_id').notNull().references(() => tenants.tenant_id, { onDelete: 'cascade' }),
  email_normalized: text('email_normalized').notNull(),
  role_default:     text('role_default').notNull().default('member'),
  app_defaults:     jsonb('app_defaults').notNull().default([]),
  expires_at:       timestamp('expires_at', { withTimezone: true, mode: 'date' }),
  created_at:       timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
}, (t) => [unique().on(t.tenant_id, t.email_normalized)]);

// ─── 14. User-owned provider keys ────────────────────────────────────────────

export const userProviderKeys = pgTable('user_provider_keys', {
  key_id:        text('key_id').primaryKey(),
  tenant_id:     text('tenant_id').notNull().references(() => tenants.tenant_id, { onDelete: 'cascade' }),
  app_id:        text('app_id').notNull().references(() => apps.app_id, { onDelete: 'cascade' }),
  user_id:       text('user_id').notNull().references(() => users.user_id, { onDelete: 'cascade' }),
  provider:      text('provider').notNull(),
  key_label:     text('key_label').notNull(),
  encrypted_key: text('encrypted_key').notNull(),
  metadata:      jsonb('metadata').notNull().default({}),
  created_at:    timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  updated_at:    timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
}, (t) => [unique().on(t.tenant_id, t.app_id, t.user_id, t.provider, t.key_label)]);

// ─── 15. CLI auth challenges ─────────────────────────────────────────────────

export const cliAuthChallenges = pgTable('cli_auth_challenges', {
  challenge_id:  text('challenge_id').primaryKey(),
  nonce:         text('nonce').notNull().unique(),
  status:        text('status').notNull().default('pending'), // pending | approved | denied | expired
  device_name:   text('device_name'),
  user_id:       text('user_id').references(() => users.user_id),
  tenant_id:     text('tenant_id').references(() => tenants.tenant_id),
  session_token: text('session_token'),
  expires_at:    timestamp('expires_at', { withTimezone: true, mode: 'date' }).notNull(),
  approved_at:   timestamp('approved_at', { withTimezone: true, mode: 'date' }),
  created_at:    timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 15b. CLI passkey credentials ───────────────────────────────────────────

export const cliPasskeyCredentials = pgTable('cli_passkey_credentials', {
  credential_id: text('credential_id').primaryKey(),
  user_id:       text('user_id').notNull().references(() => users.user_id, { onDelete: 'cascade' }),
  device_name:   text('device_name').notNull(),
  public_key:    text('public_key').notNull(),
  algorithm:     text('algorithm').notNull().default('ed25519'),
  status:        text('status').notNull().default('active'), // active | revoked
  created_at:    timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  last_used_at:  timestamp('last_used_at', { withTimezone: true, mode: 'date' }),
  revoked_at:    timestamp('revoked_at', { withTimezone: true, mode: 'date' }),
}, (t) => [unique().on(t.user_id, t.device_name)]);

// ─── 16. Founder signing keys ─────────────────────────────────────────────────

export const founderSigningKeys = pgTable('founder_signing_keys', {
  key_id:     text('key_id').primaryKey(),
  user_id:    text('user_id').notNull().references(() => users.user_id, { onDelete: 'cascade' }),
  public_key: text('public_key').notNull(),
  algorithm:  text('algorithm').notNull().default('ed25519'),
  status:     text('status').notNull().default('active'), // active | revoked
  created_at: timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  revoked_at: timestamp('revoked_at', { withTimezone: true, mode: 'date' }),
});

// ─── 17. Protected intents ────────────────────────────────────────────────────

export const protectedIntents = pgTable('protected_intents', {
  intent_id:           text('intent_id').primaryKey(),
  actor_user_id:       text('actor_user_id').notNull().references(() => users.user_id),
  tenant_id:           text('tenant_id').references(() => tenants.tenant_id),
  app_id:              text('app_id').references(() => apps.app_id),
  nonce:               text('nonce').notNull().unique(),
  payload_hash:        text('payload_hash').notNull(),
  signing_key_id:      text('signing_key_id').notNull().references(() => founderSigningKeys.key_id),
  signature:           text('signature').notNull(),
  expires_at:          timestamp('expires_at', { withTimezone: true, mode: 'date' }).notNull(),
  verification_status: text('verification_status').notNull().default('pending'), // pending | verified | rejected
  verified_at:         timestamp('verified_at', { withTimezone: true, mode: 'date' }),
  created_at:          timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 18. Protected action audit ───────────────────────────────────────────────

export const protectedActionAudit = pgTable('protected_action_audit', {
  id:               bigserial('id', { mode: 'number' }).primaryKey(),
  actor_user_id:    text('actor_user_id').notNull(),
  intent_id:        text('intent_id').references(() => protectedIntents.intent_id),
  action_type:      text('action_type').notNull(),
  payload_summary:  text('payload_summary'),
  decision:         text('decision').notNull(), // allowed | denied
  deny_reason:      text('deny_reason'),
  execution_result: text('execution_result'),
  device_info:      jsonb('device_info'),
  recorded_at:      timestamp('recorded_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 20. App service config (onboarding handshake: App → HQ credentials) ────

export const appServiceConfig = pgTable('app_service_config', {
  app_id:            text('app_id').notNull().references(() => apps.app_id, { onDelete: 'cascade' }),
  tenant_id:         text('tenant_id').notNull().references(() => tenants.tenant_id, { onDelete: 'cascade' }),
  service_url:       text('service_url'),
  api_key_encrypted: text('api_key_encrypted'),
  capabilities:      jsonb('capabilities').notNull().default([]),
  status:            text('status').notNull().default('pending'),
  onboarded_at:      timestamp('onboarded_at', { withTimezone: true, mode: 'date' }),
  onboarded_by:      text('onboarded_by').references(() => users.user_id),
  created_at:        timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  updated_at:        timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
}, (t) => [unique().on(t.app_id, t.tenant_id)]);

// ─── 21. Fuel events ledger (normalized, append-only) ───────────────────────

export const fuelEvents = pgTable('fuel_events', {
  event_id:        text('event_id').primaryKey(),
  idempotency_key: text('idempotency_key').notNull().unique(),
  tenant_id:       text('tenant_id').notNull().references(() => tenants.tenant_id),
  app_id:          text('app_id').notNull().references(() => apps.app_id),
  user_id:         text('user_id').notNull().references(() => users.user_id),
  units:           numeric('units').notNull(),
  unit_type:       text('unit_type').notNull(),
  occurred_at:     timestamp('occurred_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  source:          text('source').notNull(),
  metadata:        jsonb('metadata').default({}),
  created_at:      timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 22. Observability events (append-only, idempotent by event_id) ─────────

export const obsEvents = pgTable('obs_events', {
  event_id:     text('event_id').primaryKey(),
  event_type:   text('event_type').notNull(),
  occurred_at:  timestamp('occurred_at', { withTimezone: true, mode: 'date' }).notNull(),
  source:       text('source').notNull(),
  request_id:   text('request_id').notNull(),
  trace_id:     text('trace_id'),
  parent_event_id: text('parent_event_id'),
  intention_id: text('intention_id'),
  run_id:       text('run_id'),
  issue_id:     text('issue_id'),
  pr_id:        text('pr_id'),
  deploy_id:    text('deploy_id'),
  payload:      jsonb('payload').notNull().default({}),
  ingested_at:  timestamp('ingested_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 23. Run state projection (materialized current state per run) ─────────

export const obsRunState = pgTable('obs_run_state', {
  run_id:               text('run_id').primaryKey(),
  current_event_id:     text('current_event_id').notNull(),
  current_event_type:   text('current_event_type').notNull(),
  current_occurred_at:  timestamp('current_occurred_at', { withTimezone: true, mode: 'date' }).notNull(),
  current_source:       text('current_source').notNull(),
  current_request_id:   text('current_request_id').notNull(),
  current_trace_id:     text('current_trace_id'),
  current_parent_event_id: text('current_parent_event_id'),
  current_intention_id: text('current_intention_id'),
  current_issue_id:     text('current_issue_id'),
  current_pr_id:        text('current_pr_id'),
  current_deploy_id:    text('current_deploy_id'),
  current_payload:      jsonb('current_payload').notNull().default({}),
  last_ingested_at:     timestamp('last_ingested_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  updated_at:           timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── 24. Operational alerts (open/acked/resolved) ─────────────────────────

export const obsAlerts = pgTable('obs_alerts', {
  alert_id:       text('alert_id').primaryKey(),
  code:           text('code').notNull(),
  severity:       text('severity').notNull(), // info | warn | critical
  status:         text('status').notNull().default('open'), // open | acked | resolved
  summary:        text('summary').notNull(),
  details:        jsonb('details').notNull().default({}),
  source:         text('source'),
  intention_id:   text('intention_id'),
  run_id:         text('run_id'),
  issue_id:       text('issue_id'),
  first_seen_at:  timestamp('first_seen_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  last_seen_at:   timestamp('last_seen_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  acked_at:       timestamp('acked_at', { withTimezone: true, mode: 'date' }),
  acked_by:       text('acked_by'),
  ack_reason:     text('ack_reason'),
  resolved_at:    timestamp('resolved_at', { withTimezone: true, mode: 'date' }),
  created_at:     timestamp('created_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
  updated_at:     timestamp('updated_at', { withTimezone: true, mode: 'date' }).notNull().defaultNow(),
});

// ─── Inferred types ───────────────────────────────────────────────────────────
export type CliPasskeyCredential = typeof cliPasskeyCredentials.$inferSelect;
export type Panel              = typeof panels.$inferSelect;
export type NewPanel           = typeof panels.$inferInsert;
export type PanelComponent     = typeof panelComponents.$inferSelect;
export type NewPanelComponent  = typeof panelComponents.$inferInsert;
export type InstanceConfig     = typeof instanceConfigs.$inferSelect;
export type NewInstanceConfig  = typeof instanceConfigs.$inferInsert;
export type InstalledComponent = typeof installedComponents.$inferSelect;
export type TabMeta            = typeof tabMeta.$inferSelect;
export type PanelSettings      = typeof panelSettings.$inferSelect;
export type ChatMessage        = typeof chatMessages.$inferSelect;
export type NewChatMessage     = typeof chatMessages.$inferInsert;
export type AppSetting         = typeof appSettings.$inferSelect;
export type ServiceStatusEntry = typeof serviceStatusLog.$inferSelect;
export type User               = typeof users.$inferSelect;
export type Tenant             = typeof tenants.$inferSelect;
export type App                = typeof apps.$inferSelect;
export type TenantMembership   = typeof tenantMemberships.$inferSelect;
export type AppMembership      = typeof appMemberships.$inferSelect;
export type UserCapability     = typeof userCapabilities.$inferSelect;
export type TenantEmailAllowlistEntry = typeof tenantEmailAllowlist.$inferSelect;
export type UserProviderKey    = typeof userProviderKeys.$inferSelect;
export type CliAuthChallenge   = typeof cliAuthChallenges.$inferSelect;
export type FounderSigningKey  = typeof founderSigningKeys.$inferSelect;
export type ProtectedIntent    = typeof protectedIntents.$inferSelect;
export type ProtectedActionAuditEntry = typeof protectedActionAudit.$inferSelect;
export type AppServiceConfig  = typeof appServiceConfig.$inferSelect;
export type FuelEvent         = typeof fuelEvents.$inferSelect;
export type NewFuelEvent      = typeof fuelEvents.$inferInsert;
export type ObsEvent          = typeof obsEvents.$inferSelect;
export type NewObsEvent       = typeof obsEvents.$inferInsert;
export type ObsRunState       = typeof obsRunState.$inferSelect;
export type NewObsRunState    = typeof obsRunState.$inferInsert;
export type ObsAlert          = typeof obsAlerts.$inferSelect;
export type NewObsAlert       = typeof obsAlerts.$inferInsert;
