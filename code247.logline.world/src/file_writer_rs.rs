use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

#[derive(Clone)]
pub struct FileWriter {
    repo_root: PathBuf,
}

impl FileWriter {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    pub fn write_from_llm_output(&self, llm_output: &str) -> Result<Vec<PathBuf>> {
        let files = parse_tagged_files(llm_output);
        if files.is_empty() {
            return Err(anyhow!(
                "nenhum bloco <file path=\"...\"> encontrado na resposta do LLM"
            ));
        }

        let mut written = Vec::with_capacity(files.len());
        for (path, content) in files {
            let rel = sanitize_relative_path(&path)?;
            let full = self.repo_root.join(&rel);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full, content)?;
            written.push(rel);
        }

        Ok(written)
    }
}

fn parse_tagged_files(input: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cursor = input;

    while let Some(start_idx) = cursor.find("<file") {
        cursor = &cursor[start_idx..];
        let Some(tag_end) = cursor.find('>') else {
            break;
        };
        let opening = &cursor[..=tag_end];
        let Some(path_attr_start) = opening.find("path=\"") else {
            cursor = &cursor[tag_end + 1..];
            continue;
        };
        let after = &opening[path_attr_start + 6..];
        let Some(path_end) = after.find('"') else {
            cursor = &cursor[tag_end + 1..];
            continue;
        };
        let path = after[..path_end].trim().to_string();

        let after_tag = &cursor[tag_end + 1..];
        let Some(close_idx) = after_tag.find("</file>") else {
            break;
        };
        let body = after_tag[..close_idx].trim_matches('\n').to_string();
        out.push((path, body));

        cursor = &after_tag[close_idx + "</file>".len()..];
    }

    out
}

fn sanitize_relative_path(path: &str) -> Result<PathBuf> {
    let normalized = Path::new(path);
    if normalized.is_absolute() || path.contains("..") {
        return Err(anyhow!("path inválido para escrita: {path}"));
    }
    Ok(normalized.to_path_buf())
}
