use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ManifestValidationConfig {
    pub repo_root: PathBuf,
    pub manifest_path: PathBuf,
    pub schema_path: PathBuf,
    pub required: bool,
}

pub fn validate_manifest(cfg: &ManifestValidationConfig) -> Result<Option<PathBuf>> {
    let manifest_full = resolve_manifest_path(&cfg.repo_root, &cfg.manifest_path);

    if !manifest_full.exists() {
        if cfg.required {
            bail!(
                "manifest file is required but missing: {}",
                manifest_full.display()
            );
        }
        return Ok(None);
    }

    let schema_full = resolve_schema_path(&cfg.schema_path)?;

    let schema_raw = fs::read_to_string(&schema_full).with_context(|| {
        format!(
            "failed to read manifest schema file at {}",
            schema_full.display()
        )
    })?;
    let schema_json: Value = serde_json::from_str(&schema_raw).with_context(|| {
        format!(
            "invalid JSON in manifest schema file at {}",
            schema_full.display()
        )
    })?;

    let manifest_raw = fs::read_to_string(&manifest_full).with_context(|| {
        format!(
            "failed to read project manifest file at {}",
            manifest_full.display()
        )
    })?;
    let manifest_json: Value = serde_json::from_str(&manifest_raw).with_context(|| {
        format!(
            "invalid JSON in project manifest file at {}",
            manifest_full.display()
        )
    })?;

    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&schema_json)
        .map_err(|e| anyhow::anyhow!("failed to compile manifest schema: {e}"))?;

    if let Err(errors) = compiled.validate(&manifest_json) {
        let mut lines = Vec::new();
        for err in errors {
            lines.push(format!("- {}: {}", err.instance_path, err));
        }
        bail!(
            "project manifest validation failed against {}:\n{}",
            schema_full.display(),
            lines.join("\n")
        );
    }

    enforce_linear_contract(&manifest_json)?;

    Ok(Some(manifest_full))
}

fn resolve_manifest_path(repo_root: &Path, manifest_path: &Path) -> PathBuf {
    if manifest_path.is_absolute() {
        manifest_path.to_path_buf()
    } else {
        repo_root.join(manifest_path)
    }
}

fn resolve_schema_path(schema_path: &Path) -> Result<PathBuf> {
    if schema_path.is_absolute() {
        return Ok(schema_path.to_path_buf());
    }
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
    Ok(cwd.join(schema_path))
}

fn enforce_linear_contract(manifest_json: &Value) -> Result<()> {
    let primary = manifest_json
        .pointer("/inputs/primary")
        .and_then(Value::as_str)
        .or_else(|| {
            manifest_json
                .pointer("/task_sources/primary")
                .and_then(Value::as_str)
        });
    let linear = manifest_json
        .pointer("/inputs/linear")
        .or_else(|| manifest_json.pointer("/task_sources/linear"));

    if primary == Some("linear") && linear.is_none() {
        bail!("inputs.linear is required when inputs.primary=\"linear\"");
    }

    Ok(())
}
