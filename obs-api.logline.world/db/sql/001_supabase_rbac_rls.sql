-- Supabase RBAC + RLS baseline for UBLX / LogLine
-- Run after base tables exist.
-- Safe to re-run.

begin;

create schema if not exists app;

create or replace function app.current_user_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      current_setting('request.jwt.claim.sub', true),
      current_setting('app.current_user_id', true)
    ),
    ''
  );
$$;

create or replace function app.current_workspace_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      current_setting('request.jwt.claim.workspace_id', true),
      current_setting('app.current_workspace_id', true)
    ),
    ''
  );
$$;

create or replace function app.current_app_id()
returns text
language sql
stable
as $$
  select nullif(
    coalesce(
      current_setting('request.jwt.claim.app_id', true),
      current_setting('app.current_app_id', true)
    ),
    ''
  );
$$;

create or replace function app.is_tenant_member(target_tenant text)
returns boolean
language sql
stable
as $$
  select exists (
    select 1
    from tenant_memberships tm
    where tm.tenant_id = target_tenant
      and tm.user_id = app.current_user_id()
  );
$$;

create or replace function app.is_app_member(target_tenant text, target_app text)
returns boolean
language sql
stable
as $$
  select exists (
    select 1
    from app_memberships am
    where am.tenant_id = target_tenant
      and am.app_id = target_app
      and am.user_id = app.current_user_id()
  );
$$;

create or replace function app.is_app_admin(target_tenant text, target_app text)
returns boolean
language sql
stable
as $$
  select exists (
    select 1
    from app_memberships am
    where am.tenant_id = target_tenant
      and am.app_id = target_app
      and am.user_id = app.current_user_id()
      and am.role = 'app_admin'
  );
$$;

alter table users enable row level security;
alter table tenants enable row level security;
alter table apps enable row level security;
alter table tenant_memberships enable row level security;
alter table app_memberships enable row level security;
alter table panels enable row level security;
alter table panel_components enable row level security;
alter table instance_configs enable row level security;
alter table panel_settings enable row level security;
alter table tab_meta enable row level security;
alter table chat_messages enable row level security;
alter table app_settings enable row level security;
alter table installed_components enable row level security;
alter table service_status_log enable row level security;

-- users
drop policy if exists users_select_self on users;
create policy users_select_self on users
for select
using (user_id = app.current_user_id());

-- tenants
drop policy if exists tenants_select_member on tenants;
create policy tenants_select_member on tenants
for select
using (app.is_tenant_member(tenant_id));

-- apps
drop policy if exists apps_select_member on apps;
create policy apps_select_member on apps
for select
using (app.is_app_member(tenant_id, app_id));

-- tenant memberships: users can read their own memberships
drop policy if exists tenant_memberships_select_self on tenant_memberships;
create policy tenant_memberships_select_self on tenant_memberships
for select
using (user_id = app.current_user_id());

-- app memberships: users can read their own memberships
drop policy if exists app_memberships_select_self on app_memberships;
create policy app_memberships_select_self on app_memberships
for select
using (user_id = app.current_user_id());

-- panels
drop policy if exists panels_select_member on panels;
create policy panels_select_member on panels
for select
using (app.is_app_member(workspace_id, app_id));

drop policy if exists panels_insert_admin on panels;
create policy panels_insert_admin on panels
for insert
with check (app.is_app_admin(workspace_id, app_id));

drop policy if exists panels_update_admin on panels;
create policy panels_update_admin on panels
for update
using (app.is_app_admin(workspace_id, app_id))
with check (app.is_app_admin(workspace_id, app_id));

drop policy if exists panels_delete_admin on panels;
create policy panels_delete_admin on panels
for delete
using (app.is_app_admin(workspace_id, app_id));

-- panel components
drop policy if exists panel_components_select_member on panel_components;
create policy panel_components_select_member on panel_components
for select
using (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_components.panel_id
      and app.is_app_member(p.workspace_id, p.app_id)
  )
);

drop policy if exists panel_components_insert_admin on panel_components;
create policy panel_components_insert_admin on panel_components
for insert
with check (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_components.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

drop policy if exists panel_components_update_admin on panel_components;
create policy panel_components_update_admin on panel_components
for update
using (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_components.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
)
with check (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_components.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

drop policy if exists panel_components_delete_admin on panel_components;
create policy panel_components_delete_admin on panel_components
for delete
using (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_components.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

-- instance configs (private data): app_admin only
drop policy if exists instance_configs_select_admin on instance_configs;
create policy instance_configs_select_admin on instance_configs
for select
using (
  exists (
    select 1
    from panel_components pc
    join panels p on p.panel_id = pc.panel_id
    where pc.instance_id = instance_configs.instance_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

drop policy if exists instance_configs_insert_admin on instance_configs;
create policy instance_configs_insert_admin on instance_configs
for insert
with check (
  exists (
    select 1
    from panel_components pc
    join panels p on p.panel_id = pc.panel_id
    where pc.instance_id = instance_configs.instance_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

drop policy if exists instance_configs_update_admin on instance_configs;
create policy instance_configs_update_admin on instance_configs
for update
using (
  exists (
    select 1
    from panel_components pc
    join panels p on p.panel_id = pc.panel_id
    where pc.instance_id = instance_configs.instance_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
)
with check (
  exists (
    select 1
    from panel_components pc
    join panels p on p.panel_id = pc.panel_id
    where pc.instance_id = instance_configs.instance_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

drop policy if exists instance_configs_delete_admin on instance_configs;
create policy instance_configs_delete_admin on instance_configs
for delete
using (
  exists (
    select 1
    from panel_components pc
    join panels p on p.panel_id = pc.panel_id
    where pc.instance_id = instance_configs.instance_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

-- panel settings (private): app_admin only
drop policy if exists panel_settings_select_admin on panel_settings;
create policy panel_settings_select_admin on panel_settings
for select
using (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_settings.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

drop policy if exists panel_settings_write_admin on panel_settings;
create policy panel_settings_write_admin on panel_settings
for all
using (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_settings.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
)
with check (
  exists (
    select 1
    from panels p
    where p.panel_id = panel_settings.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

-- tab meta
drop policy if exists tab_meta_select_member on tab_meta;
create policy tab_meta_select_member on tab_meta
for select
using (
  exists (
    select 1
    from panels p
    where p.panel_id = tab_meta.panel_id
      and app.is_app_member(p.workspace_id, p.app_id)
  )
);

drop policy if exists tab_meta_write_admin on tab_meta;
create policy tab_meta_write_admin on tab_meta
for all
using (
  exists (
    select 1
    from panels p
    where p.panel_id = tab_meta.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
)
with check (
  exists (
    select 1
    from panels p
    where p.panel_id = tab_meta.panel_id
      and app.is_app_admin(p.workspace_id, p.app_id)
  )
);

-- chat messages: read member, write app_admin
drop policy if exists chat_messages_select_member on chat_messages;
create policy chat_messages_select_member on chat_messages
for select
using (app.is_app_member(workspace_id, app_id));

drop policy if exists chat_messages_insert_admin on chat_messages;
create policy chat_messages_insert_admin on chat_messages
for insert
with check (app.is_app_admin(workspace_id, app_id));

drop policy if exists chat_messages_update_admin on chat_messages;
create policy chat_messages_update_admin on chat_messages
for update
using (app.is_app_admin(workspace_id, app_id))
with check (app.is_app_admin(workspace_id, app_id));

drop policy if exists chat_messages_delete_admin on chat_messages;
create policy chat_messages_delete_admin on chat_messages
for delete
using (app.is_app_admin(workspace_id, app_id));

-- app settings (private): app_admin only, key-scoped
drop policy if exists app_settings_select_admin on app_settings;
create policy app_settings_select_admin on app_settings
for select
using (
  key like ('ws:' || app.current_workspace_id() || ':app:' || app.current_app_id() || ':%')
  and app.is_app_admin(app.current_workspace_id(), app.current_app_id())
);

drop policy if exists app_settings_write_admin on app_settings;
create policy app_settings_write_admin on app_settings
for all
using (
  key like ('ws:' || app.current_workspace_id() || ':app:' || app.current_app_id() || ':%')
  and app.is_app_admin(app.current_workspace_id(), app.current_app_id())
)
with check (
  key like ('ws:' || app.current_workspace_id() || ':app:' || app.current_app_id() || ':%')
  and app.is_app_admin(app.current_workspace_id(), app.current_app_id())
);

-- installed components: read member, write app_admin
drop policy if exists installed_components_select_member on installed_components;
create policy installed_components_select_member on installed_components
for select
using (
  component_id like (app.current_workspace_id() || ':' || app.current_app_id() || '::%')
  and app.is_app_member(app.current_workspace_id(), app.current_app_id())
);

drop policy if exists installed_components_write_admin on installed_components;
create policy installed_components_write_admin on installed_components
for all
using (
  component_id like (app.current_workspace_id() || ':' || app.current_app_id() || '::%')
  and app.is_app_admin(app.current_workspace_id(), app.current_app_id())
)
with check (
  component_id like (app.current_workspace_id() || ':' || app.current_app_id() || '::%')
  and app.is_app_admin(app.current_workspace_id(), app.current_app_id())
);

-- service status logs
drop policy if exists service_status_select_member on service_status_log;
create policy service_status_select_member on service_status_log
for select
using (app.is_app_member(workspace_id, app_id));

drop policy if exists service_status_insert_admin on service_status_log;
create policy service_status_insert_admin on service_status_log
for insert
with check (app.is_app_admin(workspace_id, app_id));

drop policy if exists service_status_update_admin on service_status_log;
create policy service_status_update_admin on service_status_log
for update
using (app.is_app_admin(workspace_id, app_id))
with check (app.is_app_admin(workspace_id, app_id));

drop policy if exists service_status_delete_admin on service_status_log;
create policy service_status_delete_admin on service_status_log
for delete
using (app.is_app_admin(workspace_id, app_id));

commit;
