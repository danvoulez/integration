mod commands;
mod integrations;
mod supabase;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::{CommandFactory, Parser, Subcommand};
use logline_api::{Intent, RuntimeEngine};
use logline_core::{
    default_config_dir, demo_catalog, load_catalog_from_dir, write_default_config_files,
};
use logline_runtime::LoglineRuntime;

use crate::commands::auth_session;
use crate::commands::cicd;
use crate::commands::db;
use crate::commands::deploy;
use crate::commands::dev;
use crate::commands::harness;
use crate::commands::secrets;
use crate::supabase::{
    SupabaseClient, SupabaseConfig, StoredAuth,
    get_valid_token, load_auth, save_auth, delete_auth,
    load_passkey, save_passkey,
};

#[derive(Debug, Parser)]
#[command(name = "logline", about = "Logline CLI — one binary, Supabase direct")]
struct Cli {
    #[arg(long, global = true)]
    json: bool,

    #[arg(long, global = true)]
    config_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        #[arg(long)]
        force: bool,
    },
    Status,
    Run {
        #[arg(long)]
        intent: String,
        #[arg(long = "arg", value_parser = parse_key_val)]
        args: Vec<(String, String)>,
    },
    Stop { run_id: String },
    Events {
        #[arg(long)]
        since: Option<String>,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
    Backend {
        #[command(subcommand)]
        command: BackendCommands,
    },
    /// CLI command catalog
    Catalog {
        #[command(subcommand)]
        command: CatalogCommands,
    },
    /// Authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// Founder operations (bootstrap, signing)
    Founder {
        #[command(subcommand)]
        command: FounderCommands,
    },
    /// App management (create, handshake, config)
    App {
        #[command(subcommand)]
        command: AppCommands,
    },
    /// Tenant management
    Tenant {
        #[command(subcommand)]
        command: TenantCommands,
    },
    /// Fuel ledger
    Fuel {
        #[command(subcommand)]
        command: FuelCommands,
    },
    /// Supabase CLI helper commands
    Supabase {
        #[command(subcommand)]
        command: SupabaseCommands,
    },
    /// Credential vault — store/retrieve secrets in macOS Keychain
    Secrets {
        #[command(subcommand)]
        command: secrets::SecretsCommands,
    },
    /// Database operations (query, tables, migrations, RLS verification)
    Db {
        #[command(subcommand)]
        command: db::DbCommands,
    },
    /// Development commands (build, start, migrate with injected credentials)
    Dev {
        #[command(subcommand)]
        command: dev::DevCommands,
    },
    /// Deploy to production (supabase, github, vercel, or all)
    Deploy {
        #[command(subcommand)]
        command: deploy::DeployCommands,
    },
    /// CI/CD pipeline runner (reads logline.cicd.json)
    Cicd {
        #[command(subcommand)]
        command: cicd::CicdCommands,
    },
    /// Severe integration harness (contracts + scenarios + auditable report)
    Harness {
        #[command(subcommand)]
        command: harness::HarnessCommands,
    },
    /// Object storage (Supabase Storage)
    Storage {
        #[command(subcommand)]
        command: StorageCommands,
    },
    /// Realtime broadcast
    Broadcast {
        #[command(subcommand)]
        command: BroadcastCommands,
    },
    /// Interactive wizard to onboard a new app into the ecosystem
    Onboard {
        /// App ID (will prompt if not provided)
        #[arg(long)]
        app_id: Option<String>,
        /// Skip interactive prompts and use defaults
        #[arg(long)]
        yes: bool,
    },
    /// Pre-flight check: vault + session + identity + pipeline readiness
    Ready {
        /// Pipeline to check readiness for
        #[arg(long, default_value = "prod")]
        pipeline: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProfileCommands {
    List,
    Use { profile_id: String },
}

#[derive(Debug, Subcommand)]
enum BackendCommands {
    List,
    Test { backend_id: String },
}

#[derive(Debug, Subcommand)]
enum CatalogCommands {
    /// Export CLI command catalog as JSON
    Export {
        /// Optional output path; prints to stdout when omitted
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum AuthCommands {
    /// Unlock session with Touch ID (required before any privileged command)
    Unlock {
        /// Session TTL (e.g. "5m", "30m", "2h"). Default: 30m
        #[arg(long, default_value = "30m")]
        ttl: String,
    },
    /// Lock session immediately (revoke access)
    Lock,
    /// Show session status and remaining TTL
    Status,
    /// Login with email/password (Supabase Auth direct)
    Login {
        /// Email address
        #[arg(long)]
        email: Option<String>,
        /// Use passkey (Touch ID) to unlock stored refresh token
        #[arg(long)]
        passkey: bool,
    },
    /// Register a passkey (Ed25519 keypair + Touch ID gate)
    PasskeyRegister {
        /// Device name for this passkey
        #[arg(long)]
        device_name: Option<String>,
    },
    /// Show current identity
    Whoami,
    /// Remove stored tokens and logout
    Logout,
    /// Debug auth/RLS - show JWT claims and test PostgREST
    Debug,
}

#[derive(Debug, Subcommand)]
enum FounderCommands {
    /// One-time world bootstrap (creates tenant, user, memberships, founder cap)
    Bootstrap {
        /// Tenant slug for the HQ tenant
        #[arg(long)]
        tenant_slug: String,
        /// Tenant display name
        #[arg(long)]
        tenant_name: String,
    },
}

#[derive(Debug, Subcommand)]
enum AppCommands {
    /// Register a new app under the current tenant
    Create {
        #[arg(long)]
        app_id: String,
        #[arg(long)]
        name: String,
    },
    /// Bidirectional handshake: store the app's service URL and API key
    Handshake {
        #[arg(long)]
        app_id: String,
        #[arg(long)]
        service_url: String,
        #[arg(long)]
        api_key: Option<String>,
        /// Comma-separated capabilities
        #[arg(long)]
        capabilities: Option<String>,
    },
    /// Export ecosystem config JSON for an app to consume
    ConfigExport {
        #[arg(long)]
        app_id: String,
    },
    /// List apps in the current tenant
    List,
    /// Issue a long-lived service token for app-to-app authentication
    IssueServiceToken {
        #[arg(long)]
        app_id: String,
        /// Token TTL in days (default: 30)
        #[arg(long, default_value = "30")]
        ttl_days: u32,
        /// Comma-separated capabilities (e.g. "llm:call,fuel:emit")
        #[arg(long)]
        capabilities: Option<String>,
        /// Human-readable description
        #[arg(long)]
        description: Option<String>,
    },
    /// Revoke a service token
    RevokeServiceToken {
        #[arg(long)]
        token_id: String,
    },
    /// List service tokens for an app
    ListServiceTokens {
        #[arg(long)]
        app_id: String,
    },
    /// Mark an app as trusted (can use billing delegation)
    Trust {
        #[arg(long)]
        app_id: String,
        /// Trust level: standard, elevated, system
        #[arg(long, default_value = "standard")]
        level: String,
    },
}

#[derive(Debug, Subcommand)]
enum TenantCommands {
    /// Create a new tenant (founder only)
    Create {
        #[arg(long)]
        slug: String,
        #[arg(long)]
        name: String,
    },
    /// Add an email to the tenant allowlist
    AllowlistAdd {
        #[arg(long)]
        email: String,
        #[arg(long, default_value = "member")]
        role: String,
        /// Comma-separated app:role pairs (e.g. "ublx:member,llm-gateway:member")
        #[arg(long)]
        app_defaults: Option<String>,
    },
    /// Resolve tenant by slug
    Resolve {
        #[arg(long)]
        slug: String,
    },
}

#[derive(Debug, Subcommand)]
enum FuelCommands {
    /// Emit a fuel event
    Emit {
        #[arg(long)]
        app_id: String,
        #[arg(long)]
        units: f64,
        #[arg(long)]
        unit_type: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// List fuel events (optionally filtered)
    List {
        /// Filter by app
        #[arg(long)]
        app_id: Option<String>,
        /// Filter by unit type
        #[arg(long)]
        unit_type: Option<String>,
        /// Max results
        #[arg(long, default_value = "50")]
        limit: u32,
    },
    /// Get fuel summary (totals by app/unit_type)
    Summary {
        /// Filter by app
        #[arg(long)]
        app_id: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum SupabaseCommands {
    /// Store Supabase access token in OS keychain (never on disk)
    StoreToken,
    Check {
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    Projects {
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    Link {
        #[arg(long)]
        project_ref: String,
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    Migrate {
        #[arg(long)]
        workdir: Option<PathBuf>,
    },
    Raw {
        #[arg(long)]
        workdir: Option<PathBuf>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum StorageCommands {
    /// Upload a file to storage
    Upload {
        /// Local file path
        file: PathBuf,
        /// Storage bucket
        #[arg(long, default_value = "artifacts")]
        bucket: String,
        /// Remote path (defaults to filename)
        #[arg(long)]
        path: Option<String>,
    },
    /// Download a file from storage
    Download {
        /// Remote path in bucket
        remote_path: String,
        /// Storage bucket
        #[arg(long, default_value = "artifacts")]
        bucket: String,
        /// Local destination (defaults to filename)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// List files in a bucket
    List {
        /// Storage bucket
        #[arg(long, default_value = "artifacts")]
        bucket: String,
        /// Path prefix to filter
        #[arg(long)]
        prefix: Option<String>,
    },
    /// Generate a signed URL for a file
    Sign {
        /// Remote path in bucket
        remote_path: String,
        /// Storage bucket
        #[arg(long, default_value = "artifacts")]
        bucket: String,
        /// URL expiration in seconds
        #[arg(long, default_value = "3600")]
        expires_in: u64,
    },
    /// Delete a file from storage
    Delete {
        /// Remote path in bucket
        remote_path: String,
        /// Storage bucket
        #[arg(long, default_value = "artifacts")]
        bucket: String,
    },
}

#[derive(Debug, Subcommand)]
enum BroadcastCommands {
    /// Send a message to a realtime channel
    Send {
        /// Channel name
        channel: String,
        /// Event type
        #[arg(long, default_value = "message")]
        event: String,
        /// JSON payload
        payload: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg_dir = cli.config_dir.clone().unwrap_or_else(default_config_dir);

    let catalog = match load_catalog_from_dir(&cfg_dir) {
        Ok(c) => c,
        Err(_) => demo_catalog(),
    };
    let runtime = LoglineRuntime::from_catalog(catalog.clone())?;

    match cli.command {
        Commands::Init { force } => {
            if force && cfg_dir.exists() {
                for name in ["connections.toml", "runtime.toml", "ui.toml"] {
                    let p = cfg_dir.join(name);
                    if p.exists() {
                        fs::remove_file(&p)?;
                    }
                }
            }
            write_default_config_files(&cfg_dir)?;
            pout(cli.json, serde_json::json!({"message":"init complete","config_dir":cfg_dir}), "Init complete")?;
        }
        Commands::Status => {
            let status = runtime.status()?;
            pout(cli.json, serde_json::to_value(status)?, "Runtime status retrieved")?;
        }
        Commands::Run { intent, args } => {
            let payload = BTreeMap::from_iter(args);
            let result = runtime.run_intent(Intent { intent_type: intent, payload })?;
            pout(cli.json, serde_json::to_value(result)?, "Intent accepted")?;
        }
        Commands::Stop { run_id } => {
            runtime.stop_run(run_id.clone())?;
            pout(cli.json, serde_json::json!({"ok":true,"run_id":run_id}), "Stop signal sent")?;
        }
        Commands::Events { since } => {
            let events = runtime.events_since(since)?;
            pout(cli.json, serde_json::to_value(events)?, "Events fetched")?;
        }
        Commands::Profile { command } => match command {
            ProfileCommands::List => {
                let profiles: Vec<_> = catalog.profiles.keys().cloned().collect();
                pout(cli.json, serde_json::to_value(profiles)?, "Profiles listed")?;
            }
            ProfileCommands::Use { profile_id } => {
                runtime.select_profile(profile_id.clone())?;
                pout(cli.json, serde_json::json!({"ok":true,"active_profile":profile_id}), "Profile selected")?;
            }
        },
        Commands::Backend { command } => match command {
            BackendCommands::List => {
                let backends: Vec<_> = catalog.backends.keys().cloned().collect();
                pout(cli.json, serde_json::to_value(backends)?, "Backends listed")?;
            }
            BackendCommands::Test { backend_id } => {
                runtime.test_backend(backend_id.clone())?;
                pout(cli.json, serde_json::json!({"ok":true,"backend_id":backend_id}), "Backend health check passed")?;
            }
        },
        Commands::Catalog { command } => match command {
            CatalogCommands::Export { output } => {
                cmd_catalog_export(output.as_ref(), cli.json)?;
            }
        },

        // ─── Auth ───────────────────────────────────────────────────────
        Commands::Auth { command } => {
            match &command {
                AuthCommands::Unlock { ttl } => {
                    return auth_session::cmd_auth_session(
                        auth_session::SessionCommands::Unlock { ttl: ttl.clone() },
                        cli.json,
                    );
                }
                AuthCommands::Lock => {
                    return auth_session::cmd_auth_session(
                        auth_session::SessionCommands::Lock,
                        cli.json,
                    );
                }
                AuthCommands::Status => {
                    return auth_session::cmd_auth_session(
                        auth_session::SessionCommands::Status,
                        cli.json,
                    );
                }
                _ => {}
            }

            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                AuthCommands::Unlock { .. } | AuthCommands::Lock | AuthCommands::Status => unreachable!(),
                AuthCommands::Login { email, passkey } => {
                    if passkey {
                        cmd_login_passkey(&client, cli.json)?;
                    } else {
                        let email = email.ok_or_else(|| {
                            anyhow::anyhow!("--email <address> is required.\nUsage: logline auth login --email you@example.com")
                        })?;
                        cmd_login_email(&client, &email, cli.json)?;
                    }
                }
                AuthCommands::PasskeyRegister { device_name } => {
                    cmd_passkey_register(&client, device_name, cli.json)?;
                }
                AuthCommands::Whoami => {
                    cmd_whoami(&client, cli.json)?;
                }
                AuthCommands::Logout => {
                    delete_auth()?;
                    pout(cli.json, serde_json::json!({"ok":true}), "Logged out. All local tokens removed.")?;
                }
                AuthCommands::Debug => {
                    cmd_auth_debug(&client, cli.json)?;
                }
            }
        }

        // ─── Founder ────────────────────────────────────────────────────
        Commands::Founder { command } => {
            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                FounderCommands::Bootstrap { tenant_slug, tenant_name } => {
                    cmd_founder_bootstrap(&client, &tenant_slug, &tenant_name, cli.json)?;
                }
            }
        }

        // ─── App ────────────────────────────────────────────────────────
        Commands::App { command } => {
            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                AppCommands::Create { app_id, name } => {
                    cmd_app_create(&client, &app_id, &name, cli.json)?;
                }
                AppCommands::Handshake { app_id, service_url, api_key, capabilities } => {
                    cmd_app_handshake(&client, &app_id, &service_url, api_key.as_deref(), capabilities.as_deref(), cli.json)?;
                }
                AppCommands::ConfigExport { app_id } => {
                    cmd_app_config_export(&client, &app_id, cli.json)?;
                }
                AppCommands::List => {
                    cmd_app_list(&client, cli.json)?;
                }
                AppCommands::IssueServiceToken { app_id, ttl_days, capabilities, description } => {
                    cmd_app_issue_service_token(&client, &app_id, ttl_days, capabilities.as_deref(), description.as_deref(), cli.json)?;
                }
                AppCommands::RevokeServiceToken { token_id } => {
                    cmd_app_revoke_service_token(&client, &token_id, cli.json)?;
                }
                AppCommands::ListServiceTokens { app_id } => {
                    cmd_app_list_service_tokens(&client, &app_id, cli.json)?;
                }
                AppCommands::Trust { app_id, level } => {
                    cmd_app_trust(&client, &app_id, &level, cli.json)?;
                }
            }
        }

        // ─── Tenant ─────────────────────────────────────────────────────
        Commands::Tenant { command } => {
            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                TenantCommands::Create { slug, name } => {
                    cmd_tenant_create(&client, &slug, &name, cli.json)?;
                }
                TenantCommands::AllowlistAdd { email, role, app_defaults } => {
                    cmd_tenant_allowlist_add(&client, &email, &role, app_defaults.as_deref(), cli.json)?;
                }
                TenantCommands::Resolve { slug } => {
                    cmd_tenant_resolve(&client, &slug, cli.json)?;
                }
            }
        }

        // ─── Fuel ───────────────────────────────────────────────────────
        Commands::Fuel { command } => {
            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                FuelCommands::Emit { app_id, units, unit_type, source, idempotency_key } => {
                    cmd_fuel_emit(&client, &app_id, units, &unit_type, &source, idempotency_key.as_deref(), cli.json)?;
                }
                FuelCommands::List { app_id, unit_type, limit } => {
                    cmd_fuel_list(&client, app_id.as_deref(), unit_type.as_deref(), limit, cli.json)?;
                }
                FuelCommands::Summary { app_id } => {
                    cmd_fuel_summary(&client, app_id.as_deref(), cli.json)?;
                }
            }
        }

        // ─── New CLI-Only commands ──────────────────────────────────────
        Commands::Secrets { command } => {
            return secrets::cmd_secrets(command, cli.json);
        }
        Commands::Db { command } => {
            return db::cmd_db(command, cli.json);
        }
        Commands::Dev { command } => {
            return dev::cmd_dev(command, cli.json);
        }
        Commands::Deploy { command } => {
            return deploy::cmd_deploy(command, cli.json);
        }
        Commands::Cicd { command } => {
            return cicd::cmd_cicd(command, cli.json);
        }
        Commands::Harness { command } => {
            return harness::cmd_harness(command, cli.json);
        }

        // ─── Storage ────────────────────────────────────────────────────
        Commands::Storage { command } => {
            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                StorageCommands::Upload { file, bucket, path } => {
                    cmd_storage_upload(&client, &file, &bucket, path.as_deref(), cli.json)?;
                }
                StorageCommands::Download { remote_path, bucket, output } => {
                    cmd_storage_download(&client, &remote_path, &bucket, output.as_deref(), cli.json)?;
                }
                StorageCommands::List { bucket, prefix } => {
                    cmd_storage_list(&client, &bucket, prefix.as_deref(), cli.json)?;
                }
                StorageCommands::Sign { remote_path, bucket, expires_in } => {
                    cmd_storage_sign(&client, &remote_path, &bucket, expires_in, cli.json)?;
                }
                StorageCommands::Delete { remote_path, bucket } => {
                    cmd_storage_delete(&client, &remote_path, &bucket, cli.json)?;
                }
            }
        }

        // ─── Broadcast ──────────────────────────────────────────────────
        Commands::Broadcast { command } => {
            let config = SupabaseConfig::from_env_or_file()?;
            let client = SupabaseClient::new(config)?;

            match command {
                BroadcastCommands::Send { channel, event, payload } => {
                    cmd_broadcast_send(&client, &channel, &event, &payload, cli.json)?;
                }
            }
        }

        // ─── Onboard ────────────────────────────────────────────────────
        Commands::Onboard { app_id, yes } => {
            return cmd_onboard(app_id.as_deref(), yes, cli.json);
        }

        Commands::Ready { pipeline } => {
            return cmd_ready(&pipeline, cli.json);
        }

        // ─── Supabase CLI helpers (legacy) ──────────────────────────────
        Commands::Supabase { command } => match command {
            SupabaseCommands::StoreToken => {
                let token = rpassword::prompt_password("Supabase Access Token (paste, hidden): ")?;
                if token.trim().is_empty() {
                    anyhow::bail!("Token cannot be empty");
                }
                let entry = keyring::Entry::new("logline-cli", "supabase_access_token")
                    .map_err(|e| anyhow::anyhow!("Keychain error: {e}"))?;
                entry.set_password(token.trim())
                    .map_err(|e| anyhow::anyhow!("Failed to store in keychain: {e}"))?;
                pout(cli.json, serde_json::json!({"ok": true}), "Supabase access token stored in OS keychain.")?;
            }
            SupabaseCommands::Check { workdir } => {
                println!("supabase version:");
                run_supabase_stream(&["--version"], workdir.as_ref())?;
                println!("\nProjects:");
                run_supabase_stream(&["projects", "list"], workdir.as_ref())?;
            }
            SupabaseCommands::Projects { workdir } => {
                run_supabase_stream(&["projects", "list"], workdir.as_ref())?;
            }
            SupabaseCommands::Link { project_ref, workdir } => {
                run_supabase_stream(&["link", "--project-ref", &project_ref], workdir.as_ref())?;
            }
            SupabaseCommands::Migrate { workdir } => {
                run_supabase_stream(&["db", "push"], workdir.as_ref())?;
            }
            SupabaseCommands::Raw { workdir, args } => {
                let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                run_supabase_stream(&str_args, workdir.as_ref())?;
            }
        },
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Command implementations
// ═══════════════════════════════════════════════════════════════════════════

fn cmd_catalog_export(output: Option<&PathBuf>, json: bool) -> anyhow::Result<()> {
    let catalog = build_cli_catalog_json();
    let serialized = serde_json::to_string_pretty(&catalog)?;

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{serialized}\n"))?;
        pout(
            json,
            serde_json::json!({"ok": true, "output": path, "catalog_version": "logline-cli.catalog.v1"}),
            &format!("Catalog exported to {}", path.display()),
        )?;
        return Ok(());
    }

    println!("{serialized}");
    Ok(())
}

fn build_cli_catalog_json() -> serde_json::Value {
    let command = Cli::command();
    serde_json::json!({
        "binary": "logline",
        "catalog_version": "logline-cli.catalog.v1",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "commands": collect_cli_command_entries("logline".to_string(), &command),
    })
}

fn collect_cli_command_entries(path: String, command: &clap::Command) -> Vec<serde_json::Value> {
    let base_path = path.clone();
    let args = command
        .get_arguments()
        .map(|arg| {
            serde_json::json!({
                "id": arg.get_id().to_string(),
                "long": arg.get_long().map(ToString::to_string),
                "short": arg.get_short().map(|v| v.to_string()),
                "required": arg.is_required_set(),
                "help": arg.get_help().map(ToString::to_string),
            })
        })
        .collect::<Vec<_>>();

    let subcommand_names = command
        .get_subcommands()
        .map(|sub| format!("{} {}", base_path, sub.get_name()))
        .collect::<Vec<_>>();

    let mut entries = vec![serde_json::json!({
        "path": path,
        "about": command.get_about().map(ToString::to_string),
        "args": args,
        "subcommands": subcommand_names,
    })];

    for subcommand in command.get_subcommands() {
        let child_path = format!("{} {}", base_path, subcommand.get_name());
        entries.extend(collect_cli_command_entries(child_path, subcommand));
    }

    entries
}

fn cmd_login_email(client: &SupabaseClient, email: &str, json: bool) -> anyhow::Result<()> {
    let password = rpassword::prompt_password(format!("Password for {email}: "))?;
    if password.is_empty() {
        anyhow::bail!("Password cannot be empty");
    }

    let resp = client.login_email(email, &password)?;
    let now = now_secs();

    let stored = StoredAuth {
        access_token: resp.access_token,
        refresh_token: resp.refresh_token,
        user_id: Some(resp.user.id.clone()),
        email: resp.user.email.clone(),
        expires_at: Some(now + resp.expires_in),
        auth_method: Some("password".into()),
    };
    save_auth(&stored)?;

    pout(json, serde_json::json!({
        "ok": true,
        "user_id": resp.user.id,
        "email": resp.user.email,
        "auth_method": "password",
    }), &format!("Logged in as {} ({})", resp.user.email.as_deref().unwrap_or("?"), resp.user.id))?;

    Ok(())
}

fn cmd_login_passkey(client: &SupabaseClient, json: bool) -> anyhow::Result<()> {
    let auth = load_auth().ok_or_else(|| {
        anyhow::anyhow!("No stored session. Run `logline auth login --email` first, then register a passkey.")
    })?;

    if load_passkey().is_none() {
        anyhow::bail!("No passkey registered. Run `logline auth passkey-register` first.");
    }

    // Touch ID gate (macOS)
    if cfg!(target_os = "macos") {
        eprintln!("Touch ID required to unlock session...");
        let result = std::process::Command::new("swift")
            .arg("-e")
            .arg(r#"
import LocalAuthentication
import Foundation
let ctx = LAContext()
var err: NSError?
guard ctx.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &err) else {
    fputs("biometrics unavailable: \(err?.localizedDescription ?? "unknown")\n", stderr)
    exit(1)
}
let sema = DispatchSemaphore(value: 0)
var ok = false
ctx.evaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, localizedReason: "Logline CLI authentication") { success, _ in
    ok = success
    sema.signal()
}
sema.wait()
exit(ok ? 0 : 1)
"#)
            .output();

        match result {
            Ok(out) if out.status.success() => {}
            Ok(_) => anyhow::bail!("Touch ID authentication failed or was cancelled."),
            Err(e) => {
                eprintln!("Touch ID unavailable ({e}), falling back to Enter confirmation.");
                eprint!("Press Enter to confirm identity: ");
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf)?;
            }
        }
    } else {
        eprint!("Press Enter to confirm identity: ");
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;
    }

    let resp = client.refresh_token(&auth.refresh_token)?;
    let now = now_secs();

    let stored = StoredAuth {
        access_token: resp.access_token.clone(),
        refresh_token: resp.refresh_token,
        user_id: Some(resp.user.id.clone()),
        email: resp.user.email.clone(),
        expires_at: Some(now + resp.expires_in),
        auth_method: Some("passkey".into()),
    };
    save_auth(&stored)?;

    pout(json, serde_json::json!({
        "ok": true,
        "user_id": resp.user.id,
        "email": resp.user.email,
        "auth_method": "passkey",
    }), &format!("Authenticated via passkey as {}", resp.user.email.as_deref().unwrap_or(&resp.user.id)))?;

    Ok(())
}

fn cmd_passkey_register(client: &SupabaseClient, device_name: Option<String>, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot determine user_id"))?;

    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let public_key = signing_key.verifying_key();
    let public_key_hex = hex::encode(public_key.as_bytes());
    let private_key_hex = hex::encode(signing_key.to_bytes());

    let device = device_name.unwrap_or_else(get_hostname);

    let passkey_data = serde_json::json!({
        "device_name": device,
        "private_key": private_key_hex,
        "public_key": public_key_hex,
        "algorithm": "ed25519",
    });

    save_passkey(&passkey_data)?;

    // Register public key in cli_passkey_credentials via PostgREST
    let cred = serde_json::json!({
        "user_id": user_id,
        "device_name": device,
        "public_key": public_key_hex,
        "algorithm": "ed25519",
        "status": "active",
    });

    client.postgrest_upsert("cli_passkey_credentials", &cred, "user_id,device_name", &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "device_name": device,
        "public_key": public_key_hex,
    }), &format!("Passkey registered for device '{}'\nPublic key: {}", device, public_key_hex))?;

    Ok(())
}

fn cmd_whoami(client: &SupabaseClient, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&user)?);
    } else {
        let id = user["id"].as_str().unwrap_or("?");
        let email = user["email"].as_str().unwrap_or("?");
        println!("User ID: {id}");
        println!("Email:   {email}");
    }

    Ok(())
}

fn cmd_auth_debug(client: &SupabaseClient, json_out: bool) -> anyhow::Result<()> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    eprintln!("=== Auth Debug ===\n");

    // 1. Check stored auth
    let auth = load_auth();
    match &auth {
        Some(a) => {
            eprintln!("✓ Stored auth found");
            eprintln!("  user_id:     {}", a.user_id.as_deref().unwrap_or("?"));
            eprintln!("  email:       {}", a.email.as_deref().unwrap_or("?"));
            eprintln!("  auth_method: {}", a.auth_method.as_deref().unwrap_or("?"));
            eprintln!("  expires_at:  {:?}", a.expires_at);
        }
        None => {
            eprintln!("✗ No stored auth found");
            return Ok(());
        }
    }

    let auth = auth.unwrap();
    let token = &auth.access_token;

    // 2. Decode JWT claims (without verification - just to see structure)
    eprintln!("\n=== JWT Claims ===\n");
    let mut jwt_sub: Option<String> = None;
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() >= 2 {
        match URL_SAFE_NO_PAD.decode(parts[1]) {
            Ok(payload) => {
                if let Ok(claims) = serde_json::from_slice::<serde_json::Value>(&payload) {
                    jwt_sub = claims["sub"].as_str().map(String::from);
                    eprintln!("{}", serde_json::to_string_pretty(&claims)?);
                } else {
                    eprintln!("Could not parse JWT payload as JSON");
                }
            }
            Err(e) => eprintln!("Could not decode JWT payload: {e}"),
        }
    } else {
        eprintln!("Invalid JWT structure (expected 3 parts, got {})", parts.len());
    }

    // 3. Test Auth API (GoTrue - uses Authorization header directly)
    eprintln!("\n=== Auth API Test (GoTrue) ===\n");
    match client.get_user(token) {
        Ok(user) => {
            let id = user["id"].as_str().unwrap_or("?");
            let email = user["email"].as_str().unwrap_or("?");
            eprintln!("✓ Auth API works: {email} ({id})");
        }
        Err(e) => eprintln!("✗ Auth API failed: {e}"),
    }

    // 4. Test PostgREST - call a simple RPC to verify JWT claim extraction
    eprintln!("\n=== PostgREST JWT Claim Test ===\n");
    // We'll query tenant_memberships and see if rows come back
    // If app.current_user_id() works, we'll get our membership
    // If not, we'll get empty results (RLS blocks)
    
    let user_id = jwt_sub.as_deref().or(auth.user_id.as_deref()).unwrap_or("");
    eprintln!("Testing with user_id: {user_id}");
    
    match client.postgrest_get("tenant_memberships", &format!("select=tenant_id,role&user_id=eq.{user_id}&limit=1"), token) {
        Ok(rows) => {
            let arr = rows.as_array();
            if arr.map(|a| a.is_empty()).unwrap_or(true) {
                eprintln!("\n✗ No rows returned (RLS blocking)");
                eprintln!("\n  The membership exists in the database but RLS prevents access.");
                eprintln!("  This happens when app.current_user_id() returns NULL.");
                eprintln!("\n  CAUSE: Supabase changed how JWT claims are exposed to PostgREST:");
                eprintln!("    Old: request.jwt.claim.sub (individual settings)");
                eprintln!("    New: request.jwt.claims (JSON object)");
                eprintln!("\n  FIX: Deploy migration 009 to update JWT claim extraction:");
                eprintln!("       logline db migrate apply --env production");
            } else {
                let tenant = arr.unwrap()[0]["tenant_id"].as_str().unwrap_or("?");
                let role = arr.unwrap()[0]["role"].as_str().unwrap_or("?");
                eprintln!("✓ RLS working! Got membership: tenant={tenant}, role={role}");
            }
        }
        Err(e) => eprintln!("✗ Query failed: {e}"),
    }

    if json_out {
        println!("{}", serde_json::json!({"ok": true}));
    }

    Ok(())
}

fn cmd_founder_bootstrap(
    client: &SupabaseClient,
    tenant_slug: &str,
    tenant_name: &str,
    json: bool,
) -> anyhow::Result<()> {
    let service_role_key = std::env::var("SUPABASE_SERVICE_ROLE_KEY")
        .map_err(|_| anyhow::anyhow!(
            "SUPABASE_SERVICE_ROLE_KEY env var required for bootstrap.\n\
             This is a one-time operation. The service role key is never needed again."
        ))?;

    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot determine user_id from JWT"))?;
    let email = user["email"].as_str().unwrap_or("");
    let display_name = user["user_metadata"]["display_name"].as_str().unwrap_or(email);

    eprintln!("Bootstrapping world as {email} ({user_id})...");

    let tenant_id = tenant_slug.to_string();

    // All inserts use service-role key to bypass RLS (nothing exists yet)
    client.service_role_insert("users", &serde_json::json!({
        "user_id": user_id,
        "email": email,
        "display_name": display_name,
    }), &service_role_key)?;
    eprintln!("  ✓ User record created");

    client.service_role_insert("tenants", &serde_json::json!({
        "tenant_id": tenant_id,
        "slug": tenant_slug,
        "name": tenant_name,
    }), &service_role_key)?;
    eprintln!("  ✓ Tenant '{tenant_slug}' created");

    client.service_role_insert("tenant_memberships", &serde_json::json!({
        "tenant_id": tenant_id,
        "user_id": user_id,
        "role": "admin",
    }), &service_role_key)?;
    eprintln!("  ✓ Tenant membership (admin)");

    client.service_role_insert("user_capabilities", &serde_json::json!({
        "user_id": user_id,
        "capability": "founder",
        "granted_by": user_id,
    }), &service_role_key)?;
    eprintln!("  ✓ Founder capability granted");

    client.service_role_insert("apps", &serde_json::json!({
        "app_id": "ublx",
        "tenant_id": tenant_id,
        "name": "UBLX Headquarters",
    }), &service_role_key)?;
    eprintln!("  ✓ HQ app 'ublx' created");

    client.service_role_insert("app_memberships", &serde_json::json!({
        "app_id": "ublx",
        "tenant_id": tenant_id,
        "user_id": user_id,
        "role": "app_admin",
    }), &service_role_key)?;
    eprintln!("  ✓ App membership (app_admin)");

    eprintln!();
    eprintln!("WARNING: Consider rotating SUPABASE_SERVICE_ROLE_KEY in the");
    eprintln!("  Supabase Dashboard -> Settings -> API -> Service Role Key.");
    eprintln!("  The key used for bootstrap should not be reused.");

    pout(json, serde_json::json!({
        "ok": true,
        "tenant_id": tenant_id,
        "user_id": user_id,
        "app_id": "ublx",
    }), &format!(
        "\nBootstrap complete.\n\
         Tenant: {tenant_slug} ({tenant_name})\n\
         Founder: {email}\n\
         HQ App: ublx\n\n\
         Service role key is no longer needed. All operations now use JWT + RLS."
    ))?;

    Ok(())
}

fn cmd_app_create(client: &SupabaseClient, app_id: &str, name: &str, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot determine user_id"))?;

    // Get first tenant membership to determine tenant_id
    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id,role&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found. Run `logline founder bootstrap` first."))?;

    client.postgrest_insert("apps", &serde_json::json!({
        "app_id": app_id,
        "tenant_id": tenant_id,
        "name": name,
    }), &token)?;

    client.postgrest_insert("app_memberships", &serde_json::json!({
        "app_id": app_id,
        "tenant_id": tenant_id,
        "user_id": user_id,
        "role": "app_admin",
    }), &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "app_id": app_id,
        "tenant_id": tenant_id,
    }), &format!("App '{name}' ({app_id}) created under tenant {tenant_id}"))?;

    Ok(())
}

fn cmd_app_handshake(
    client: &SupabaseClient,
    app_id: &str,
    service_url: &str,
    api_key: Option<&str>,
    capabilities: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().unwrap_or("?");

    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found"))?;

    let caps: Vec<String> = capabilities
        .map(|c| c.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let body = serde_json::json!({
        "app_id": app_id,
        "tenant_id": tenant_id,
        "service_url": service_url,
        "api_key_encrypted": api_key.unwrap_or(""),
        "capabilities": caps,
        "status": "active",
        "onboarded_at": chrono_now(),
        "onboarded_by": user_id,
    });

    client.postgrest_upsert("app_service_config", &body, "app_id,tenant_id", &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "app_id": app_id,
        "service_url": service_url,
        "capabilities": caps,
    }), &format!("Handshake complete for '{app_id}'.\nHQ can now reach {service_url}"))?;

    Ok(())
}

fn cmd_app_config_export(client: &SupabaseClient, app_id: &str, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().unwrap_or("?");

    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .unwrap_or("?");

    let config = serde_json::json!({
        "supabase_url": client.config.url,
        "supabase_anon_key": client.config.anon_key,
        "app_id": app_id,
        "tenant_id": tenant_id,
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!("Ecosystem config for '{app_id}':\n");
        println!("{}", serde_json::to_string_pretty(&config)?);
        println!("\nPaste this into the app's configuration.");
    }

    Ok(())
}

fn cmd_app_list(client: &SupabaseClient, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let apps = client.postgrest_get("apps", "select=app_id,tenant_id,name,created_at", &token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&apps)?);
    } else if let Some(arr) = apps.as_array() {
        if arr.is_empty() {
            println!("No apps found.");
        } else {
            for app in arr {
                println!("  {} — {} (tenant: {})",
                    app["app_id"].as_str().unwrap_or("?"),
                    app["name"].as_str().unwrap_or("?"),
                    app["tenant_id"].as_str().unwrap_or("?"),
                );
            }
        }
    }

    Ok(())
}

fn cmd_app_issue_service_token(
    client: &SupabaseClient,
    app_id: &str,
    ttl_days: u32,
    capabilities: Option<&str>,
    description: Option<&str>,
    _json: bool,
) -> anyhow::Result<()> {
    use jsonwebtoken::{encode, Header, EncodingKey, Algorithm};
    use sha2::{Sha256, Digest};
    use chrono::{Utc, Duration};

    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("No user id"))?;

    // Get app to verify ownership and get tenant_id
    let apps = client.postgrest_get(
        "apps",
        &format!("select=app_id,tenant_id&app_id=eq.{}", app_id),
        &token,
    )?;
    let app = apps.as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow::anyhow!("App not found: {}", app_id))?;
    let tenant_id = app["tenant_id"].as_str()
        .ok_or_else(|| anyhow::anyhow!("App has no tenant_id"))?;

    // Get JWT secret from env
    let jwt_secret = std::env::var("SUPABASE_JWT_SECRET")
        .map_err(|_| anyhow::anyhow!("SUPABASE_JWT_SECRET not set"))?;

    // Parse comma-separated capabilities
    let caps: Vec<String> = capabilities
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
        .unwrap_or_default();

    // Build claims
    let now = Utc::now();
    let exp = now + Duration::days(ttl_days as i64);
    
    let claims = serde_json::json!({
        "sub": app_id,
        "tenant_id": tenant_id,
        "role": "service",
        "capabilities": caps,
        "iat": now.timestamp(),
        "exp": exp.timestamp(),
    });

    // Sign JWT
    let service_jwt = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )?;

    // Hash for storage (to allow revocation without storing plaintext)
    let mut hasher = Sha256::new();
    hasher.update(service_jwt.as_bytes());
    let token_hash = hex::encode(hasher.finalize());

    // Store in database
    let record = serde_json::json!({
        "app_id": app_id,
        "tenant_id": tenant_id,
        "token_hash": token_hash,
        "capabilities": caps,
        "expires_at": exp.to_rfc3339(),
        "description": description.map(|s| s.to_string()),
        "issued_by": user_id,
    });
    client.postgrest_insert("service_tokens", &record, &token)?;

    println!("Service token issued for {}.", app_id);
    println!("Expires: {}", exp.format("%Y-%m-%d %H:%M:%S UTC"));
    println!();
    println!("TOKEN (save it now, it won't be shown again):");
    println!("{}", service_jwt);

    Ok(())
}

fn cmd_app_revoke_service_token(
    client: &SupabaseClient,
    token_id: &str,
    _json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    // Call the revoke function
    let result = client.postgrest_rpc(
        "revoke_service_token",
        &serde_json::json!({ "p_token_id": token_id }),
        &token,
    )?;

    if result.as_bool() == Some(true) {
        println!("Service token {} revoked.", token_id);
    } else {
        println!("Token not found or already revoked.");
    }

    Ok(())
}

fn cmd_app_list_service_tokens(
    client: &SupabaseClient,
    app_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    let query = format!("select=*&app_id=eq.{}", app_id);
    let tokens = client.postgrest_get("v_active_service_tokens", &query, &token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&tokens)?);
    } else if let Some(arr) = tokens.as_array() {
        if arr.is_empty() {
            println!("No active service tokens found.");
        } else {
            for t in arr {
                println!("  {} — app: {}, expires: {}, caps: {:?}",
                    t["id"].as_str().unwrap_or("?"),
                    t["app_id"].as_str().unwrap_or("?"),
                    t["expires_at"].as_str().unwrap_or("?"),
                    t["capabilities"],
                );
            }
        }
    }

    Ok(())
}

fn cmd_app_trust(
    client: &SupabaseClient,
    app_id: &str,
    level: &str,
    _json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    // Validate trust level
    let valid_levels = ["none", "billing_delegation", "full"];
    if !valid_levels.contains(&level) {
        anyhow::bail!("Invalid trust level. Valid: {:?}", valid_levels);
    }

    if level == "none" {
        // Remove from trusted_apps
        client.postgrest_delete(
            "trusted_apps",
            &format!("app_id=eq.{}", app_id),
            &token,
        )?;
        println!("App {} removed from trusted apps.", app_id);
    } else {
        // Upsert into trusted_apps
        let record = serde_json::json!({
            "app_id": app_id,
            "trust_level": level,
        });
        // Try delete first, then insert (simple upsert)
        let _ = client.postgrest_delete("trusted_apps", &format!("app_id=eq.{}", app_id), &token);
        client.postgrest_insert("trusted_apps", &record, &token)?;
        println!("App {} set to trust level: {}", app_id, level);
    }

    Ok(())
}

fn cmd_tenant_create(client: &SupabaseClient, slug: &str, name: &str, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    client.postgrest_insert("tenants", &serde_json::json!({
        "tenant_id": slug,
        "slug": slug,
        "name": name,
    }), &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "tenant_id": slug,
        "slug": slug,
        "name": name,
    }), &format!("Tenant '{name}' ({slug}) created"))?;

    Ok(())
}

fn cmd_tenant_allowlist_add(
    client: &SupabaseClient,
    email: &str,
    role: &str,
    app_defaults: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().unwrap_or("?");

    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found"))?;

    let defaults: Vec<serde_json::Value> = app_defaults
        .map(|ad| {
            ad.split(',')
                .filter_map(|pair| {
                    let mut parts = pair.trim().splitn(2, ':');
                    let app = parts.next()?;
                    let r = parts.next().unwrap_or("member");
                    Some(serde_json::json!({"app_id": app, "role": r}))
                })
                .collect()
        })
        .unwrap_or_default();

    let email_norm = email.trim().to_lowercase();

    client.postgrest_upsert("tenant_email_allowlist", &serde_json::json!({
        "tenant_id": tenant_id,
        "email_normalized": email_norm,
        "role_default": role,
        "app_defaults": defaults,
    }), "tenant_id,email_normalized", &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "email": email_norm,
        "tenant_id": tenant_id,
        "role": role,
        "app_defaults": defaults,
    }), &format!("Added {email_norm} to allowlist (role: {role})"))?;

    Ok(())
}

fn cmd_tenant_resolve(client: &SupabaseClient, slug: &str, json: bool) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let tenants = client.postgrest_get("tenants", &format!("select=tenant_id,slug,name,created_at&slug=eq.{slug}"), &token)?;

    let tenant = tenants.as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow::anyhow!("Tenant with slug '{slug}' not found"))?;

    if json {
        println!("{}", serde_json::to_string_pretty(tenant)?);
    } else {
        println!("Tenant: {} ({})", tenant["name"].as_str().unwrap_or("?"), tenant["tenant_id"].as_str().unwrap_or("?"));
    }

    Ok(())
}

fn cmd_fuel_emit(
    client: &SupabaseClient,
    app_id: &str,
    units: f64,
    unit_type: &str,
    source: &str,
    idempotency_key: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot determine user_id"))?;

    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found"))?;

    let idem_key = idempotency_key
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}-{}-{}-{}", app_id, user_id, unit_type, now_secs()));

    client.postgrest_insert("fuel_events", &serde_json::json!({
        "idempotency_key": idem_key,
        "tenant_id": tenant_id,
        "app_id": app_id,
        "user_id": user_id,
        "units": units,
        "unit_type": unit_type,
        "source": source,
    }), &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "idempotency_key": idem_key,
        "app_id": app_id,
        "units": units,
        "unit_type": unit_type,
    }), &format!("Fuel event emitted: {units} {unit_type} for {app_id}"))?;

    Ok(())
}

fn cmd_fuel_list(
    client: &SupabaseClient,
    app_id: Option<&str>,
    unit_type: Option<&str>,
    limit: u32,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot determine user_id"))?;

    // Get tenant_id from membership
    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found"))?;

    // Build filter query
    let mut filters = format!("tenant_id=eq.{tenant_id}");
    if let Some(app) = app_id {
        filters.push_str(&format!("&app_id=eq.{app}"));
    }
    if let Some(ut) = unit_type {
        filters.push_str(&format!("&unit_type=eq.{ut}"));
    }
    let query = format!("select=*&{filters}&order=created_at.desc&limit={limit}");

    let events = client.postgrest_get("fuel_events", &query, &token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&events)?);
    } else {
        let empty = vec![];
        let arr = events.as_array().unwrap_or(&empty);
        println!("Fuel events ({} results):\n", arr.len());
        for e in arr {
            let app = e["app_id"].as_str().unwrap_or("?");
            let units = e["units"].as_f64().unwrap_or(0.0);
            let ut = e["unit_type"].as_str().unwrap_or("?");
            let src = e["source"].as_str().unwrap_or("?");
            let ts = e["created_at"].as_str().unwrap_or("?");
            println!("  {ts}  {app:<20} {units:>10.2} {ut:<15} ({src})");
        }
    }
    Ok(())
}

fn cmd_fuel_summary(
    client: &SupabaseClient,
    app_id: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;
    let user = client.get_user(&token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot determine user_id"))?;

    // Get tenant_id from membership
    let memberships = client.postgrest_get("tenant_memberships", &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"), &token)?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found"))?;

    // Query all events and aggregate in-memory (PostgREST doesn't support GROUP BY)
    let mut filters = format!("tenant_id=eq.{tenant_id}");
    if let Some(app) = app_id {
        filters.push_str(&format!("&app_id=eq.{app}"));
    }
    let query = format!("select=app_id,unit_type,units&{filters}&limit=10000");
    let events = client.postgrest_get("fuel_events", &query, &token)?;

    // Aggregate: (app_id, unit_type) -> total
    let mut totals: std::collections::BTreeMap<(String, String), f64> = std::collections::BTreeMap::new();
    if let Some(arr) = events.as_array() {
        for e in arr {
            let app = e["app_id"].as_str().unwrap_or("unknown").to_string();
            let ut = e["unit_type"].as_str().unwrap_or("unknown").to_string();
            let units = e["units"].as_f64().unwrap_or(0.0);
            *totals.entry((app, ut)).or_insert(0.0) += units;
        }
    }

    if json {
        let summary: Vec<_> = totals.iter()
            .map(|((app, ut), total)| serde_json::json!({
                "app_id": app,
                "unit_type": ut,
                "total": total,
            }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        println!("Fuel Summary:\n");
        println!("  {:<20} {:<15} {:>12}", "APP", "UNIT TYPE", "TOTAL");
        println!("  {}", "-".repeat(50));
        for ((app, ut), total) in &totals {
            println!("  {:<20} {:<15} {:>12.2}", app, ut, total);
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Storage
// ═══════════════════════════════════════════════════════════════════════════

fn cmd_storage_upload(
    client: &SupabaseClient,
    file: &std::path::Path,
    bucket: &str,
    path: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    let filename = file.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;
    let remote_path = path.unwrap_or(filename);

    let content = std::fs::read(file)
        .map_err(|e| anyhow::anyhow!("Failed to read file: {e}"))?;

    let content_type = mime_guess::from_path(file)
        .first_or_octet_stream()
        .to_string();

    client.storage_upload(bucket, remote_path, &content, &content_type, &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "bucket": bucket,
        "path": remote_path,
        "size": content.len(),
    }), &format!("Uploaded {filename} to {bucket}/{remote_path}"))?;

    Ok(())
}

fn cmd_storage_download(
    client: &SupabaseClient,
    remote_path: &str,
    bucket: &str,
    output: Option<&std::path::Path>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    let content = client.storage_download(bucket, remote_path, &token)?;

    let filename = remote_path.rsplit('/').next().unwrap_or(remote_path);
    let out_path = output.unwrap_or(std::path::Path::new(filename));

    std::fs::write(out_path, &content)
        .map_err(|e| anyhow::anyhow!("Failed to write file: {e}"))?;

    pout(json, serde_json::json!({
        "ok": true,
        "bucket": bucket,
        "path": remote_path,
        "output": out_path,
        "size": content.len(),
    }), &format!("Downloaded {bucket}/{remote_path} to {}", out_path.display()))?;

    Ok(())
}

fn cmd_storage_list(
    client: &SupabaseClient,
    bucket: &str,
    prefix: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    let files = client.storage_list(bucket, prefix, &token)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&files)?);
    } else {
        let empty = vec![];
        let arr = files.as_array().unwrap_or(&empty);
        println!("Files in {bucket}{}:", prefix.map(|p| format!("/{p}")).unwrap_or_default());
        for f in arr {
            let name = f["name"].as_str().unwrap_or("?");
            let size = f["metadata"]["size"].as_u64().unwrap_or(0);
            println!("  {name:<40} {size:>10} bytes");
        }
    }
    Ok(())
}

fn cmd_storage_sign(
    client: &SupabaseClient,
    remote_path: &str,
    bucket: &str,
    expires_in: u64,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    let signed_url = client.storage_sign(bucket, remote_path, expires_in, &token)?;

    pout(json, serde_json::json!({
        "url": signed_url,
        "expires_in": expires_in,
    }), &signed_url)?;

    Ok(())
}

fn cmd_storage_delete(
    client: &SupabaseClient,
    remote_path: &str,
    bucket: &str,
    json: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    client.storage_delete(bucket, remote_path, &token)?;

    pout(json, serde_json::json!({
        "ok": true,
        "bucket": bucket,
        "path": remote_path,
    }), &format!("Deleted {bucket}/{remote_path}"))?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Broadcast
// ═══════════════════════════════════════════════════════════════════════════

fn cmd_broadcast_send(
    client: &SupabaseClient,
    channel: &str,
    event: &str,
    payload: &str,
    json_output: bool,
) -> anyhow::Result<()> {
    let token = get_valid_token(client)?;

    let payload: serde_json::Value = serde_json::from_str(payload)
        .map_err(|e| anyhow::anyhow!("Invalid JSON payload: {e}"))?;

    client.broadcast(channel, event, &payload, &token)?;

    pout(json_output, serde_json::json!({
        "ok": true,
        "channel": channel,
        "event": event,
    }), &format!("Broadcast sent to channel '{channel}' event '{event}'"))?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Onboard (interactive wizard)
// ═══════════════════════════════════════════════════════════════════════════

fn cmd_onboard(app_id_arg: Option<&str>, skip_prompts: bool, json: bool) -> anyhow::Result<()> {
    use std::io::{self, Write};

    // Require unlocked session
    crate::require_unlocked()?;

    let config = SupabaseConfig::from_env_or_file()?;
    let client = SupabaseClient::new(config)?;
    let token = get_valid_token(&client)?;

    if !json {
        println!("\n🚀 Logline App Onboarding\n");
    }

    // Helper for prompts
    let prompt = |label: &str, default: Option<&str>| -> anyhow::Result<String> {
        if skip_prompts {
            return default.map(|s| s.to_string()).ok_or_else(|| {
                anyhow::anyhow!("--yes requires all values to be provided or have defaults")
            });
        }
        let suffix = default.map(|d| format!(" [{}]", d)).unwrap_or_default();
        print!("{}{}: ", label, suffix);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            default.map(|s| s.to_string()).ok_or_else(|| {
                anyhow::anyhow!("{} is required", label)
            })
        } else {
            Ok(trimmed.to_string())
        }
    };

    // 1. Gather app info
    let app_id = match app_id_arg {
        Some(id) => id.to_string(),
        None => prompt("App ID (e.g., code247)", None)?,
    };
    let app_name = prompt("Display Name", Some(&app_id))?;
    let service_url = prompt(
        "Service URL",
        Some(&format!("https://{}.logline.world", app_id)),
    )?;
    let local_port = prompt("Local Port", Some("8080"))?;

    // 2. Capabilities selection
    let available_caps = [
        "job:submit", "job:status", "llm:call", "llm:stream",
        "storage:read", "storage:write", "fuel:emit", "broadcast:send",
    ];
    if !json && !skip_prompts {
        println!("\nAvailable capabilities:");
        for cap in &available_caps {
            println!("  - {}", cap);
        }
    }
    let caps_input = prompt("Capabilities (comma-separated)", Some("fuel:emit"))?;

    // 3. Execute steps
    let mut results: Vec<serde_json::Value> = Vec::new();

    // Step 1: Create app
    if !json { print!("\n📝 Registering app... "); io::stdout().flush()?; }
    match cmd_app_create_inner(&client, &app_id, &app_name, &token) {
        Ok(_) => {
            if !json { println!("✅"); }
            results.push(serde_json::json!({"step": "create", "ok": true}));
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("duplicate") || msg.contains("already exists") {
                if !json { println!("⏭️  (already exists)"); }
                results.push(serde_json::json!({"step": "create", "ok": true, "skipped": true}));
            } else {
                if !json { println!("❌ {}", e); }
                results.push(serde_json::json!({"step": "create", "ok": false, "error": msg}));
            }
        }
    }

    // Step 2: Handshake
    if !json { print!("🤝 Performing handshake... "); io::stdout().flush()?; }
    match cmd_app_handshake_inner(&client, &app_id, &service_url, None, Some(&caps_input), &token) {
        Ok(_) => {
            if !json { println!("✅"); }
            results.push(serde_json::json!({"step": "handshake", "ok": true}));
        }
        Err(e) => {
            if !json { println!("❌ {}", e); }
            results.push(serde_json::json!({"step": "handshake", "ok": false, "error": e.to_string()}));
        }
    }

    // Step 3: Export config
    let config_path = format!("./{}/config.env", app_id);
    if !json { print!("📄 Exporting config to {}... ", config_path); io::stdout().flush()?; }
    match cmd_app_config_export_to_file(&client, &app_id, &config_path, &token) {
        Ok(_) => {
            if !json { println!("✅"); }
            results.push(serde_json::json!({"step": "config_export", "ok": true, "path": config_path}));
        }
        Err(e) => {
            if !json { println!("❌ {}", e); }
            results.push(serde_json::json!({"step": "config_export", "ok": false, "error": e.to_string()}));
        }
    }

    // Step 4: PM2 ecosystem entry
    if !json { print!("⚙️  Adding PM2 ecosystem entry... "); io::stdout().flush()?; }
    match add_pm2_entry(&app_id, &local_port) {
        Ok(path) => {
            if !json { println!("✅"); }
            results.push(serde_json::json!({"step": "pm2_config", "ok": true, "path": path}));
        }
        Err(e) => {
            if !json { println!("⚠️  {} (manual setup needed)", e); }
            results.push(serde_json::json!({"step": "pm2_config", "ok": false, "error": e.to_string()}));
        }
    }

    // Step 5: Cloudflare tunnel hint
    if !json {
        println!("🌐 Cloudflare tunnel: add route for {}.logline.world -> localhost:{}", app_id, local_port);
        results.push(serde_json::json!({"step": "cloudflare_hint", "ok": true}));
    }

    // Summary
    let all_ok = results.iter().all(|r| r["ok"].as_bool().unwrap_or(false));
    
    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "app_id": app_id,
            "service_url": service_url,
            "steps": results,
            "success": all_ok,
        }))?);
    } else {
        println!("\n{}", "─".repeat(50));
        if all_ok {
            println!("✅ Onboarding complete!\n");
            println!("Next steps:");
            println!("  1. cd {}", app_id);
            println!("  2. Review config.env and copy to .env.local");
            println!("  3. pm2 start ecosystem.config.cjs --only {}", app_id);
            println!("  4. logline app list  # verify registration");
        } else {
            println!("⚠️  Onboarding completed with warnings. Review steps above.");
        }
    }

    Ok(())
}

// Helper functions for onboard
fn cmd_app_create_inner(
    client: &SupabaseClient,
    app_id: &str,
    name: &str,
    token: &str,
) -> anyhow::Result<()> {
    let user = client.get_user(token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot get user_id"))?;

    // Get tenant from membership
    let memberships = client.postgrest_get(
        "tenant_memberships",
        &format!("select=tenant_id&user_id=eq.{user_id}&limit=1"),
        token,
    )?;
    let tenant_id = memberships.as_array()
        .and_then(|a| a.first())
        .and_then(|m| m["tenant_id"].as_str())
        .ok_or_else(|| anyhow::anyhow!("No tenant membership found"))?;

    client.postgrest_insert("apps", &serde_json::json!({
        "id": app_id,
        "tenant_id": tenant_id,
        "name": name,
        "status": "active",
    }), token)?;

    Ok(())
}

fn cmd_app_handshake_inner(
    client: &SupabaseClient,
    app_id: &str,
    service_url: &str,
    api_key: Option<&str>,
    capabilities: Option<&str>,
    token: &str,
) -> anyhow::Result<()> {
    let user = client.get_user(token)?;
    let user_id = user["id"].as_str().ok_or_else(|| anyhow::anyhow!("Cannot get user_id"))?;

    let caps_json: serde_json::Value = capabilities
        .map(|c| {
            let arr: Vec<&str> = c.split(',').map(|s| s.trim()).collect();
            serde_json::json!(arr)
        })
        .unwrap_or(serde_json::json!([]));

    let mut body = serde_json::json!({
        "service_url": service_url,
        "capabilities": caps_json,
        "onboarded_at": chrono_now(),
        "onboarded_by": user_id,
    });

    if let Some(key) = api_key {
        body["api_key"] = serde_json::json!(key);
    }

    // PATCH the app record
    let url = format!("{}/rest/v1/apps?id=eq.{}", client.config.url, app_id);
    let resp = reqwest::blocking::Client::new()
        .patch(&url)
        .header("apikey", &client.config.anon_key)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .header("Prefer", "return=representation")
        .json(&body)
        .send()?;

    if !resp.status().is_success() {
        anyhow::bail!("Handshake failed: {}", resp.text().unwrap_or_default());
    }

    Ok(())
}

fn cmd_app_config_export_to_file(
    client: &SupabaseClient,
    app_id: &str,
    output_path: &str,
    token: &str,
) -> anyhow::Result<()> {
    // Get app record
    let apps = client.postgrest_get("apps", &format!("select=*&id=eq.{}", app_id), token)?;
    let app = apps.as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow::anyhow!("App not found: {}", app_id))?;

    // Build env file content
    let mut content = String::new();
    content.push_str(&format!("# Logline ecosystem config for {}\n", app_id));
    content.push_str(&format!("# Generated: {}\n\n", chrono_now()));
    content.push_str(&format!("LOGLINE_APP_ID={}\n", app_id));
    content.push_str(&format!("LOGLINE_TENANT_ID={}\n", app["tenant_id"].as_str().unwrap_or("")));
    content.push_str(&format!("NEXT_PUBLIC_SUPABASE_URL={}\n", client.config.url));
    content.push_str(&format!("NEXT_PUBLIC_SUPABASE_ANON_KEY={}\n", client.config.anon_key));

    if let Some(url) = app["service_url"].as_str() {
        content.push_str(&format!("SERVICE_URL={}\n", url));
    }

    // Create directory if needed
    if let Some(parent) = std::path::Path::new(output_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(output_path, content)?;
    Ok(())
}

fn add_pm2_entry(app_id: &str, port: &str) -> anyhow::Result<String> {
    let ecosystem_path = "ecosystem.config.cjs";
    
    // Check if file exists
    if !std::path::Path::new(ecosystem_path).exists() {
        // Create new ecosystem file
        let content = format!(r#"module.exports = {{
  apps: [
    {{
      name: '{}',
      script: './target/release/{}',
      cwd: './{app_id}',
      env: {{
        PORT: '{}',
        RUST_LOG: 'info',
      }},
      watch: false,
      autorestart: true,
    }},
  ],
}};
"#, app_id, app_id, port, app_id = app_id);
        std::fs::write(ecosystem_path, content)?;
        return Ok(ecosystem_path.to_string());
    }

    // File exists - just notify user to add manually
    anyhow::bail!("ecosystem.config.cjs exists - add {} entry manually", app_id)
}

fn chrono_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ═══════════════════════════════════════════════════════════════════════════
// Ready (pre-flight)
// ═══════════════════════════════════════════════════════════════════════════

fn cmd_ready(pipeline: &str, json: bool) -> anyhow::Result<()> {
    use commands::auth_session;

    let mut issues: Vec<String> = Vec::new();

    // 1. Session
    let session_ok = auth_session::load_session()
        .is_some_and(|s| {
            s.expires_at > std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });
    if !session_ok {
        issues.push("Session locked. Fix: logline auth unlock".into());
    }

    // 2. Auth identity
    let identity = auth_session::load_identity();
    let logged_in = identity.is_some();
    let passkey_ok = identity.as_ref().is_some_and(|i| i.auth_method == "passkey");
    let founder_blocked = identity.as_ref().is_some_and(|i| i.is_founder);

    if !logged_in {
        issues.push("Not logged in. Fix: logline auth login --passkey".into());
    } else if !passkey_ok {
        issues.push(format!(
            "Auth method is '{}', must be 'passkey'. Fix: logline auth login --passkey",
            identity.as_ref().map(|i| i.auth_method.as_str()).unwrap_or("?")
        ));
    }
    if founder_blocked {
        issues.push("Founder/god mode blocked for infra. Fix: use operator/service account.".into());
    }

    // 3. Pipeline exists
    let pipeline_file = std::env::current_dir()
        .unwrap_or_default()
        .join("logline.cicd.json");
    let pipeline_exists = if pipeline_file.exists() {
        let content = std::fs::read_to_string(&pipeline_file).unwrap_or_default();
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&content);
        parsed
            .ok()
            .and_then(|v| v["pipelines"][pipeline].as_array().map(|a| !a.is_empty()))
            .unwrap_or(false)
    } else {
        false
    };
    if !pipeline_exists {
        issues.push(format!("Pipeline '{pipeline}' not found in logline.cicd.json"));
    }

    // 4. Key secrets
    let required_keys = ["database_url", "github_token", "vercel_token", "vercel_org_id", "vercel_project_id"];
    let mut missing_keys: Vec<&str> = Vec::new();
    for key in &required_keys {
        if secrets::load_credential(key).is_none() {
            missing_keys.push(key);
        }
    }
    if !missing_keys.is_empty() {
        issues.push(format!(
            "Missing secrets: {}. Fix: logline secrets set <key>",
            missing_keys.join(", ")
        ));
    }

    let ready = issues.is_empty();

    let report = serde_json::json!({
        "ready": ready,
        "pipeline": pipeline,
        "session_active": session_ok,
        "logged_in": logged_in,
        "passkey_ok": passkey_ok,
        "founder_blocked": founder_blocked,
        "pipeline_exists": pipeline_exists,
        "missing_secrets": missing_keys,
        "issues": issues,
    });

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("Pre-flight: {pipeline}\n");

    let items: &[(&str, bool)] = &[
        ("session", session_ok),
        ("logged_in", logged_in),
        ("passkey", passkey_ok),
        ("non-founder", !founder_blocked),
        ("pipeline", pipeline_exists),
        ("secrets", missing_keys.is_empty()),
    ];

    for (name, ok) in items {
        let mark = if *ok { "✓" } else { "✗" };
        println!("  {mark} {name}");
    }

    println!();
    if ready {
        println!("Ready. Run: logline cicd run --pipeline {pipeline}");
    } else {
        for issue in &issues {
            println!("  ✗ {issue}");
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn get_hostname() -> String {
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.is_empty() {
            return h;
        }
    }
    if let Ok(h) = fs::read_to_string("/etc/hostname") {
        let trimmed = h.trim().to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    "logline-cli".to_string()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let months: [u64; 12] = [31, if leap {29} else {28}, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0;
    for m in months {
        if days < m { break; }
        days -= m;
        month += 1;
    }
    (year, month + 1, days + 1)
}

fn is_leap(y: u64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos = s.find('=').ok_or_else(|| "must be KEY=VALUE".to_string())?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

/// Gate: require an active unlocked session. Used by command modules.
pub fn require_unlocked() -> anyhow::Result<commands::auth_session::SessionToken> {
    commands::auth_session::require_unlocked()
}

/// Uber-gate: session + passkey + non-founder. Used by deploy/cicd/db commands.
pub fn require_infra_identity() -> anyhow::Result<(commands::auth_session::SessionToken, commands::auth_session::AuthIdentity)> {
    commands::auth_session::require_infra_identity()
}

pub fn pout(json_mode: bool, value: serde_json::Value, text: &str) -> anyhow::Result<()> {
    if json_mode {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        println!("{text}");
    }
    Ok(())
}

// ─── Supabase CLI helpers ───────────────────────────────────────────────────

fn run_supabase_stream(args: &[&str], workdir: Option<&PathBuf>) -> anyhow::Result<()> {
    let mut cmd = Command::new("supabase");
    if let Some(wd) = workdir {
        cmd.arg("--workdir").arg(wd);
    }
    apply_supabase_env(&mut cmd, workdir);
    cmd.args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());

    let status = cmd.status().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow::anyhow!("supabase CLI not found. Install with `brew install supabase/tap/supabase`")
        } else {
            anyhow::anyhow!(e)
        }
    })?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("supabase command failed with status {status}");
    }
}

fn apply_supabase_env(cmd: &mut Command, _workdir: Option<&PathBuf>) {
    let has_access = std::env::var("SUPABASE_ACCESS_TOKEN")
        .ok()
        .is_some_and(|v| !v.trim().is_empty());
    if has_access {
        return;
    }

    if let Ok(entry) = keyring::Entry::new("logline-cli", "supabase_access_token") {
        if let Ok(token) = entry.get_password() {
            cmd.env("SUPABASE_ACCESS_TOKEN", token);
            return;
        }
    }

    eprintln!("Warning: No SUPABASE_ACCESS_TOKEN found in keychain or env.");
    eprintln!("  Store it with: logline supabase store-token");
}
