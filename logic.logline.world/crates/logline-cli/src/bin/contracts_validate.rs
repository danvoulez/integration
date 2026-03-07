use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(name = "contracts-validate")]
#[command(about = "Validate canonical Integration contracts (schemas, registry, policy, openapi)")]
struct Cli {
    /// Integration root path (contains contracts/ and policy/)
    #[arg(long)]
    root: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = resolve_root(cli.root)?;

    validate_json_contract(&root.join("contracts/events.registry.json"))?;
    validate_json_contract(&root.join("policy/policy-set.v1.1.json"))?;

    let schemas_dir = root.join("contracts/schemas");
    for entry in fs::read_dir(&schemas_dir)
        .with_context(|| format!("failed to read {}", schemas_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            validate_json_contract(&path)?;
        }
    }

    validate_openapi(&root.join("contracts/openapi/edge-control.v1.openapi.yaml"))?;
    validate_openapi(&root.join("contracts/openapi/inference-plane.v1.openapi.yaml"))?;
    validate_openapi(&root.join("llm-gateway.logline.world/openapi.yaml"))?;
    validate_openapi(&root.join("code247.logline.world/openapi.yaml"))?;
    validate_openapi(&root.join("obs-api.logline.world/openapi.yaml"))?;
    validate_llm_gateway_output_contract(&root.join("llm-gateway.logline.world/openapi.yaml"))?;
    validate_openapi_topology_alignment(&root)?;

    println!("contracts validation ok: {}", root.display());
    Ok(())
}

fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = root {
        return Ok(root);
    }

    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    for candidate in [cwd.clone(), cwd.join("../.."), cwd.join("../../..")] {
        if candidate.join("contracts/events.registry.json").exists() {
            return Ok(candidate
                .canonicalize()
                .with_context(|| format!("failed to canonicalize {}", candidate.display()))?);
        }
    }

    bail!("could not resolve integration root; pass --root <path>");
}

fn validate_json_contract(path: &Path) -> Result<()> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let json: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON at {}", path.display()))?;

    if json.get("$schema").is_some() {
        JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(&json)
            .map_err(|err| {
                anyhow::anyhow!("schema compilation failed for {}: {err}", path.display())
            })?;
    }

    Ok(())
}

fn validate_openapi(path: &Path) -> Result<()> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("invalid YAML at {}", path.display()))?;

    let openapi = yaml
        .get("openapi")
        .and_then(serde_yaml::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing openapi version field in {}", path.display()))?;
    if !openapi.starts_with("3.") {
        bail!(
            "unsupported openapi version {openapi} in {}",
            path.display()
        );
    }

    Ok(())
}

fn validate_llm_gateway_output_contract(path: &Path) -> Result<()> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("invalid YAML at {}", path.display()))?;

    let required = yaml
        .get("components")
        .and_then(|v| v.get("schemas"))
        .and_then(|v| v.get("ChatResponse"))
        .and_then(|v| v.get("required"))
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or_else(|| anyhow::anyhow!("missing components.schemas.ChatResponse.required"))?;

    let required_fields = required
        .iter()
        .filter_map(serde_yaml::Value::as_str)
        .collect::<Vec<_>>();

    if !required_fields.contains(&"request_id") || !required_fields.contains(&"output_schema") {
        bail!(
            "llm-gateway ChatResponse must require request_id and output_schema in {}",
            path.display()
        );
    }

    Ok(())
}

fn validate_openapi_topology_alignment(root: &Path) -> Result<()> {
    let topology_path = root.join("service_topology.json");
    let topology_raw = fs::read_to_string(&topology_path)
        .with_context(|| format!("failed to read {}", topology_path.display()))?;
    let topology: serde_json::Value = serde_json::from_str(&topology_raw)
        .with_context(|| format!("invalid JSON at {}", topology_path.display()))?;

    let ingress = topology
        .get("ingress")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing ingress array in {}", topology_path.display()))?;

    let mut host_to_local = HashMap::<String, String>::new();
    for row in ingress {
        let Some(hostname) = row.get("hostname").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(service) = row.get("service").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if let Some(local_url) = normalize_topology_local_url(service) {
            host_to_local.insert(hostname.to_string(), local_url);
        }
    }

    let checks = [
        (
            "llm-gateway.logline.world/openapi.yaml",
            "llm-gateway.logline.world",
        ),
        (
            "contracts/openapi/edge-control.v1.openapi.yaml",
            "edge-control.logline.world",
        ),
        (
            "code247.logline.world/openapi.yaml",
            "code247.logline.world",
        ),
        (
            "obs-api.logline.world/openapi.yaml",
            "obs-api.logline.world",
        ),
    ];

    for (openapi_rel, host) in checks {
        let openapi_path = root.join(openapi_rel);
        let servers = openapi_server_urls(&openapi_path)?;
        let expected_public = format!("https://{host}");
        if !servers.iter().any(|url| url == &expected_public) {
            bail!(
                "openapi/topology drift: {} missing server url {}",
                openapi_path.display(),
                expected_public
            );
        }

        let Some(expected_local) = host_to_local.get(host) else {
            bail!(
                "openapi/topology drift: missing topology ingress mapping for host '{}'",
                host
            );
        };
        if !servers.iter().any(|url| url == expected_local) {
            bail!(
                "openapi/topology drift: {} missing local server url {}",
                openapi_path.display(),
                expected_local
            );
        }
    }

    Ok(())
}

fn openapi_server_urls(path: &Path) -> Result<Vec<String>> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&raw)
        .with_context(|| format!("invalid YAML at {}", path.display()))?;
    let servers = yaml
        .get("servers")
        .and_then(serde_yaml::Value::as_sequence)
        .ok_or_else(|| anyhow::anyhow!("missing servers array in {}", path.display()))?;
    let urls = servers
        .iter()
        .filter_map(|entry| entry.get("url"))
        .filter_map(serde_yaml::Value::as_str)
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if urls.is_empty() {
        bail!("missing servers[].url entries in {}", path.display());
    }
    Ok(urls)
}

fn normalize_topology_local_url(service: &str) -> Option<String> {
    let trimmed = service.trim();
    if let Some(port) = trimmed.strip_prefix("http://127.0.0.1:") {
        return Some(format!("http://localhost:{port}"));
    }
    if let Some(port) = trimmed.strip_prefix("http://localhost:") {
        return Some(format!("http://localhost:{port}"));
    }
    None
}
