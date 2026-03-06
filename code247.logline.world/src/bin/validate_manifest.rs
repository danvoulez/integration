use std::{env, path::PathBuf};

use anyhow::Result;

#[path = "../manifest_validation_rs.rs"]
mod manifest_validation_rs;

use manifest_validation_rs::{validate_manifest, ManifestValidationConfig};

fn main() -> Result<()> {
    let _ = dotenvy::dotenv();

    let repo_root = env::var("CODE247_REPO_ROOT").unwrap_or_else(|_| ".".to_string());
    let manifest_path = env::var("CODE247_MANIFEST_PATH")
        .unwrap_or_else(|_| ".code247/workspace.manifest.json".to_string());
    let schema_path = env::var("CODE247_MANIFEST_SCHEMA_PATH")
        .unwrap_or_else(|_| "schemas/workspace.manifest.schema.json".to_string());
    let required = env_bool("CODE247_MANIFEST_REQUIRED").unwrap_or(true);

    let cfg = ManifestValidationConfig {
        repo_root: PathBuf::from(repo_root),
        manifest_path: PathBuf::from(manifest_path),
        schema_path: PathBuf::from(schema_path),
        required,
    };

    match validate_manifest(&cfg)? {
        Some(path) => {
            println!("manifest validation OK: {}", path.display());
        }
        None => {
            println!("manifest not found and not required");
        }
    }

    Ok(())
}

fn env_bool(key: &str) -> Option<bool> {
    let raw = env::var(key).ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
