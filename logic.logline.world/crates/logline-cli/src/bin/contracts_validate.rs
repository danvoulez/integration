use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
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
