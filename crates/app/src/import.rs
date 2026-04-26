//! Config import and export functionality.

use crate::error::{AppError, Result};
use crate::state::AppState;
use config_core::{ConfigLayer, ModelConfig, ModelLimit, OpenCodeConfig, ProviderConfig};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const IMPORT_META_KEY: &str = "_opmImport";
const IMPORTABLE_EXTENSIONS: &[&str] = &["json", "jsonc", "toml", "yaml", "yml"];

/// Import configuration from an external opencode.json file.
pub fn import_config(
    state: &mut AppState,
    path: &Path,
    layer: ConfigLayer,
    merge_mode: ImportMergeMode,
) -> Result<()> {
    let external_config = parse_import_path(path, None)?;
    apply_import_config(state, external_config, layer, merge_mode)
}

/// Import a JSON/JSONC/TOML/YAML snippet into an app state layer.
pub fn import_snippet(
    state: &mut AppState,
    snippet: &str,
    provider_id_hint: Option<&str>,
    source_label: Option<&str>,
    layer: ConfigLayer,
    merge_mode: ImportMergeMode,
) -> Result<ImportSummary> {
    let config = parse_import_snippet(snippet, provider_id_hint, source_label)?;
    let summary = ImportSummary::from_config(&config);
    apply_import_config(state, config, layer, merge_mode)?;
    Ok(summary)
}

/// Import from a local path or supported URL.
pub fn import_source(
    state: &mut AppState,
    source: &str,
    provider_id_hint: Option<&str>,
    layer: ConfigLayer,
    merge_mode: ImportMergeMode,
) -> Result<ImportSummary> {
    let config = parse_import_source(source, provider_id_hint)?;
    let summary = ImportSummary::from_config(&config);
    apply_import_config(state, config, layer, merge_mode)?;
    Ok(summary)
}

/// Parse an import source without mutating state.
pub fn parse_import_source(source: &str, provider_id_hint: Option<&str>) -> Result<OpenCodeConfig> {
    if is_url(source) {
        parse_import_url(source, provider_id_hint)
    } else {
        let path = Path::new(source);
        if path.exists() {
            parse_import_path(path, provider_id_hint)
        } else {
            parse_import_snippet(source, provider_id_hint, Some("inline snippet"))
        }
    }
}

/// Parse an inline JSON/JSONC/TOML/YAML import snippet.
pub fn parse_import_snippet(
    snippet: &str,
    provider_id_hint: Option<&str>,
    source_label: Option<&str>,
) -> Result<OpenCodeConfig> {
    let value = parse_loose_value(snippet)?;
    normalize_import_value(value, provider_id_hint, source_label)
}

/// Apply an already-normalized config to a layer.
pub fn apply_import_config(
    state: &mut AppState,
    external_config: OpenCodeConfig,
    layer: ConfigLayer,
    merge_mode: ImportMergeMode,
) -> Result<()> {
    match merge_mode {
        ImportMergeMode::Replace => match layer {
            ConfigLayer::Global => state.global_config = Some(external_config),
            ConfigLayer::Project => state.project_config = Some(external_config),
            ConfigLayer::Custom => state.custom_config = Some(external_config),
        },
        ImportMergeMode::Merge => {
            let target = match layer {
                ConfigLayer::Global => &mut state.global_config,
                ConfigLayer::Project => &mut state.project_config,
                ConfigLayer::Custom => &mut state.custom_config,
            };

            match target {
                Some(existing) => {
                    *target = Some(config_core::merge_two(existing.clone(), external_config));
                }
                None => {
                    *target = Some(external_config);
                }
            }
        }
    }

    state.recompute_merged();
    state.mark_dirty();
    Ok(())
}

/// Export current merged config to a file.
pub fn export_config(state: &AppState, path: &Path, export_scope: ExportScope) -> Result<()> {
    let config = match export_scope {
        ExportScope::Merged => &state.merged_config,
        ExportScope::Global => state
            .global_config
            .as_ref()
            .ok_or_else(|| AppError::State("No global config".to_string()))?,
        ExportScope::Project => state
            .project_config
            .as_ref()
            .ok_or_else(|| AppError::State("No project config".to_string()))?,
        ExportScope::Custom => state
            .custom_config
            .as_ref()
            .ok_or_else(|| AppError::State("No custom config".to_string()))?,
    };

    config_core::jsonc::write_config(config, path)?;
    Ok(())
}

fn parse_import_path(path: &Path, provider_id_hint: Option<&str>) -> Result<OpenCodeConfig> {
    if path.is_dir() {
        return parse_import_directory(path, provider_id_hint);
    }

    let content = std::fs::read_to_string(path)?;
    let hint = provider_id_hint.or_else(|| path.file_stem().and_then(|s| s.to_str()));
    parse_import_snippet(&content, hint, Some(&path.display().to_string()))
}

fn parse_import_directory(dir: &Path, provider_id_hint: Option<&str>) -> Result<OpenCodeConfig> {
    let provider_toml = dir.join("provider.toml");
    if provider_toml.exists() {
        return parse_models_dev_directory(dir, provider_id_hint);
    }

    let mut merged = OpenCodeConfig::default();
    for path in collect_importable_files(dir)? {
        let parsed = parse_import_path(&path, provider_id_hint)?;
        merged = config_core::merge_two(merged, parsed);
    }
    if merged == OpenCodeConfig::default() {
        return Err(AppError::Import(format!(
            "No importable JSON/TOML/YAML files found in {}",
            dir.display()
        )));
    }
    Ok(merged)
}

fn parse_models_dev_directory(
    dir: &Path,
    provider_id_hint: Option<&str>,
) -> Result<OpenCodeConfig> {
    let provider_id = provider_id_hint
        .map(str::to_string)
        .or_else(|| dir.file_name().and_then(|s| s.to_str()).map(str::to_string))
        .ok_or_else(|| AppError::Import("Provider directory has no usable name".to_string()))?;

    let provider_content = std::fs::read_to_string(dir.join("provider.toml"))?;
    let provider_value = parse_toml_value(&provider_content)?;
    let mut provider = models_dev_provider_from_value(provider_value)?;

    let models_dir = dir.join("models");
    if models_dir.exists() {
        for model_path in collect_importable_files(&models_dir)? {
            let model_id = model_path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| AppError::Import("Model file has no usable name".to_string()))?
                .to_string();
            let model_content = std::fs::read_to_string(&model_path)?;
            let model_value = parse_loose_value(&model_content)?;
            let model = model_from_value(model_value)?;
            provider
                .models
                .get_or_insert_with(HashMap::new)
                .insert(model_id, model);
        }
    }

    config_from_provider(provider_id, provider, Some(&dir.display().to_string()))
}

fn parse_import_url(url: &str, provider_id_hint: Option<&str>) -> Result<OpenCodeConfig> {
    // Try all possible branch/depth splits for GitHub tree URLs.
    // Branch names can contain slashes (e.g. "feat/Volcano_Engine"), so a
    // simple split would mis-parse the branch boundary.
    if let Some(candidates) = parse_github_tree_candidates(url) {
        // Try each candidate (shortest branch first) until one resolves.
        // We probe the GitHub Contents API — a 404 means wrong split.
        let mut last_err = None;
        for (owner, repo, branch, path) in candidates {
            match github_contents(&owner, &repo, &branch, &path) {
                Ok(entries) => {
                    // Found the right split — continue with the full parse
                    return parse_github_directory_with_entries(
                        &owner, &repo, &branch, &path,
                        entries, provider_id_hint, url,
                    );
                }
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            }
        }
        // All candidates failed
        return Err(last_err.unwrap_or_else(|| {
            AppError::Import("Could not resolve GitHub tree URL".to_string())
        }));
    }

    // Non-tree GitHub URL or non-GitHub URL
    if let Some((_owner, _repo, _branch, _path, is_tree)) = parse_github_url(url) {
        if !is_tree {
            // Raw file URL — just download it
            let text = http_get_text(url)?;
            let stem_hint = url_path_stem(url);
            let hint = provider_id_hint.or(stem_hint.as_deref());
            return parse_import_snippet(&text, hint, Some(url));
        }
    }

    let text = http_get_text(url)?;
    let stem_hint = url_path_stem(url);
    let hint = provider_id_hint.or(stem_hint.as_deref());
    parse_import_snippet(&text, hint, Some(url))
}

/// For a GitHub tree URL, return all possible (owner, repo, branch, path)
/// candidates ordered by shortest branch name first.
fn parse_github_tree_candidates(url: &str) -> Option<Vec<(String, String, String, String)>> {
    let rest = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 5 {
        return None;
    }
    let owner = parts[0].to_string();
    let repo = parts[1].to_string();
    let kind = parts[2];
    if kind != "tree" {
        return None;
    }

    // parts[3..] = branch_segment_1 / branch_segment_2 / ... / path_remaining
    // Try: branch = parts[3], path = parts[4..]
    //      branch = parts[3..4], path = parts[5..]
    //      etc.
    let remaining = &parts[3..];
    let mut candidates = Vec::new();
    for depth in 1..remaining.len() {
        let branch = remaining[..depth].join("/");
        let path = remaining[depth..].join("/");
        if !path.is_empty() {
            candidates.push((owner.clone(), repo.clone(), branch, path));
        }
    }
    // Sort by branch length (shortest first) to prefer simpler branch names
    candidates.sort_by_key(|c| c.2.len());
    Some(candidates)
}

fn parse_github_directory_with_entries(
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    entries: Vec<GithubContentEntry>,
    provider_id_hint: Option<&str>,
    source_url: &str,
) -> Result<OpenCodeConfig> {
    let provider_entry = entries
        .iter()
        .find(|entry| entry.name == "provider.toml" && entry.download_url.is_some());

    if let Some(provider_entry) = provider_entry {
        let provider_id = provider_id_hint
            .map(str::to_string)
            .or_else(|| path.rsplit('/').next().map(str::to_string))
            .ok_or_else(|| {
                AppError::Import("GitHub provider path has no provider ID".to_string())
            })?;

        let provider_text = http_get_text(provider_entry.download_url.as_ref().unwrap())
            .map_err(|e| {
                AppError::Import(format!(
                    "Failed to download provider.toml from GitHub: {e}"
                ))
            })?;
        let mut provider = models_dev_provider_from_value(parse_toml_value(&provider_text)?)?;

        let models_path = format!("{}/models", path.trim_end_matches('/'));
        // models/ subdirectory is optional — don't error if it doesn't exist
        if let Ok(model_entries) = github_contents(owner, repo, branch, &models_path) {
            for model_entry in model_entries {
                if model_entry.entry_type == "file" && is_importable_name(&model_entry.name) {
                    let Some(download_url) = model_entry.download_url else {
                        continue;
                    };
                    let model_id = Path::new(&model_entry.name)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .ok_or_else(|| {
                            AppError::Import("Model URL has no usable file name".to_string())
                        })?
                        .to_string();
                    let model = model_from_value(
                        parse_loose_value(
                            &http_get_text(&download_url).map_err(|e| {
                                AppError::Import(format!(
                                    "Failed to download model file {model_id}: {e}"
                                ))
                            })?,
                        )?,
                    )?;
                    provider
                        .models
                        .get_or_insert_with(HashMap::new)
                        .insert(model_id, model);
                }
            }
        }

        return config_from_provider(provider_id, provider, Some(source_url));
    }

    let mut merged = OpenCodeConfig::default();
    for entry in entries {
        if entry.entry_type == "file" && is_importable_name(&entry.name) {
            if let Some(download_url) = entry.download_url {
                let text = http_get_text(&download_url).map_err(|e| {
                    AppError::Import(format!(
                        "Failed to download {} from GitHub: {e}",
                        entry.name
                    ))
                })?;
                let parsed = parse_import_snippet(
                    &text,
                    provider_id_hint,
                    Some(&download_url),
                )?;
                merged = config_core::merge_two(merged, parsed);
            }
        }
    }
    Ok(merged)
}

fn normalize_import_value(
    value: Value,
    provider_id_hint: Option<&str>,
    source_label: Option<&str>,
) -> Result<OpenCodeConfig> {
    let mut config = if value.get("provider").is_some()
        || value.get("$schema").is_some()
        || value.get("model").is_some()
        || value.get("smallModel").is_some()
    {
        serde_json::from_value::<OpenCodeConfig>(value)?
    } else if looks_like_provider_map(&value) {
        let providers = serde_json::from_value::<HashMap<String, ProviderConfig>>(value)?;
        OpenCodeConfig {
            provider: Some(providers),
            ..Default::default()
        }
    } else if looks_like_provider(&value) {
        let provider_id = provider_id_hint.ok_or_else(|| {
            AppError::Import(
                "Provider fragment needs a provider ID hint; pass --provider-id or import from a named file/directory".to_string(),
            )
        })?;
        let provider = provider_from_value(value)?;
        config_from_provider(provider_id.to_string(), provider, source_label)?
    } else if looks_like_model(&value) {
        let model_id = provider_id_hint.ok_or_else(|| {
            AppError::Import(
                "Model fragment needs an ID hint from --provider-id/path; wrap it in a provider.models object for direct import".to_string(),
            )
        })?;
        let mut provider = ProviderConfig::default();
        provider
            .models
            .get_or_insert_with(HashMap::new)
            .insert(model_id.to_string(), model_from_value(value)?);
        config_from_provider(model_id.to_string(), provider, source_label)?
    } else {
        return Err(AppError::Import(
            "Snippet is not a full config, provider map, provider fragment, or model fragment"
                .to_string(),
        ));
    };

    attach_import_metadata(&mut config, source_label);
    Ok(config)
}

fn config_from_provider(
    provider_id: String,
    provider: ProviderConfig,
    source_label: Option<&str>,
) -> Result<OpenCodeConfig> {
    let mut providers = HashMap::new();
    providers.insert(provider_id, provider);
    let mut config = OpenCodeConfig {
        provider: Some(providers),
        ..Default::default()
    };
    attach_import_metadata(&mut config, source_label);
    Ok(config)
}

fn provider_from_value(value: Value) -> Result<ProviderConfig> {
    if is_models_dev_provider_value(&value) {
        models_dev_provider_from_value(value)
    } else {
        Ok(serde_json::from_value(value)?)
    }
}

fn models_dev_provider_from_value(value: Value) -> Result<ProviderConfig> {
    let obj = value.as_object().ok_or_else(|| {
        AppError::Import("models.dev provider metadata must be an object".to_string())
    })?;
    let mut provider = ProviderConfig {
        name: obj.get("name").and_then(Value::as_str).map(str::to_string),
        npm: obj.get("npm").and_then(Value::as_str).map(str::to_string),
        ..Default::default()
    };

    let mut options = HashMap::new();
    if let Some(api) = obj.get("api").and_then(Value::as_str) {
        options.insert("baseURL".to_string(), Value::String(api.to_string()));
    }
    if let Some(env_name) = obj
        .get("env")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
    {
        options.insert(
            "apiKey".to_string(),
            Value::String(format!("{{env:{env_name}}}")),
        );
    }
    if !options.is_empty() {
        provider.options = Some(options);
    }

    for (key, val) in obj {
        if !matches!(key.as_str(), "name" | "npm" | "api" | "env" | "models") {
            provider.extra.insert(key.clone(), val.clone());
        }
    }

    if let Some(models) = obj.get("models").and_then(Value::as_object) {
        let mut parsed_models = HashMap::new();
        for (model_id, model_value) in models {
            parsed_models.insert(model_id.clone(), model_from_value(model_value.clone())?);
        }
        provider.models = Some(parsed_models);
    }

    Ok(provider)
}

fn model_from_value(value: Value) -> Result<ModelConfig> {
    let obj = value
        .as_object()
        .ok_or_else(|| AppError::Import("Model metadata must be an object".to_string()))?;
    let mut model = ModelConfig {
        name: obj.get("name").and_then(Value::as_str).map(str::to_string),
        id: obj.get("id").and_then(Value::as_str).map(str::to_string),
        ..Default::default()
    };

    if let Some(limit) = obj.get("limit").and_then(Value::as_object) {
        model.limit = Some(ModelLimit {
            context: limit.get("context").and_then(Value::as_u64),
            output: limit.get("output").and_then(Value::as_u64),
        });
    }

    if let Some(options) = obj.get("options").and_then(Value::as_object) {
        model.options = Some(options.clone().into_iter().collect());
    }

    for (key, val) in obj {
        if !matches!(
            key.as_str(),
            "name" | "id" | "limit" | "options" | "variants" | "disabled"
        ) {
            model.extra.insert(key.clone(), val.clone());
        }
    }

    if let Some(parsed_variants) = obj.get("variants") {
        model.variants = serde_json::from_value(parsed_variants.clone())?;
    }
    model.disabled = obj.get("disabled").and_then(Value::as_bool);

    Ok(model)
}

fn parse_loose_value(content: &str) -> Result<Value> {
    let trimmed = content.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && let Ok(handler) = config_core::jsonc::JsoncHandler::parse(content)
    {
        return serde_json::from_str(&handler.to_json_string()?).map_err(AppError::from);
    }

    if let Ok(value) = toml::from_str::<toml::Value>(content) {
        return serde_json::to_value(value).map_err(AppError::from);
    }

    if let Ok(value) = serde_yaml::from_str::<Value>(content) {
        return Ok(value);
    }

    parse_toml_value(content)
}

fn parse_toml_value(content: &str) -> Result<Value> {
    let value = toml::from_str::<toml::Value>(content)
        .map_err(|e| AppError::Import(format!("Could not parse as JSON, YAML, or TOML: {e}")))?;
    serde_json::to_value(value).map_err(AppError::from)
}

fn looks_like_provider_map(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|obj| !obj.is_empty() && obj.values().all(looks_like_provider))
}

fn looks_like_provider(value: &Value) -> bool {
    value.as_object().is_some_and(|obj| {
        obj.contains_key("npm")
            || obj.contains_key("options")
            || obj.contains_key("models")
            || obj.contains_key("api")
            || obj.contains_key("env")
    })
}

fn looks_like_model(value: &Value) -> bool {
    value.as_object().is_some_and(|obj| {
        obj.contains_key("limit")
            || obj.contains_key("modalities")
            || obj.contains_key("cost")
            || obj.contains_key("family")
    })
}

fn is_models_dev_provider_value(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|obj| obj.contains_key("api") || obj.contains_key("env"))
}

fn attach_import_metadata(config: &mut OpenCodeConfig, source_label: Option<&str>) {
    let Some(source) = source_label else {
        return;
    };

    config.extra.insert(
        IMPORT_META_KEY.to_string(),
        serde_json::json!({
            "source": source,
            "note": "Imported by opencode-provider-manager. Keep this metadata as provenance for future review."
        }),
    );
}

fn collect_importable_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_importable_files(&path)?);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| IMPORTABLE_EXTENSIONS.contains(&ext))
        {
            files.push(path);
        }
    }
    Ok(files)
}

fn is_url(source: &str) -> bool {
    source.starts_with("https://") || source.starts_with("http://")
}

fn is_importable_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMPORTABLE_EXTENSIONS.contains(&ext))
}

fn http_get_text(url: &str) -> Result<String> {
    reqwest::blocking::Client::new()
        .get(url)
        .header(reqwest::header::USER_AGENT, "opencode-provider-manager")
        .send()?
        .error_for_status()?
        .text()
        .map_err(AppError::from)
}

fn parse_github_url(url: &str) -> Option<(String, String, String, String, bool)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    let kind = parts.next()?;
    let branch = parts.next()?.to_string();
    let path = parts.collect::<Vec<_>>().join("/");
    if path.is_empty() {
        return None;
    }
    Some((owner, repo, branch, path, kind == "tree"))
}

fn url_path_stem(url: &str) -> Option<String> {
    url.rsplit('/')
        .next()
        .and_then(|name| Path::new(name).file_stem())
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
}

fn github_contents(
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
) -> Result<Vec<GithubContentEntry>> {
    let api_url =
        format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}?ref={branch}");
    let response = reqwest::blocking::Client::new()
        .get(&api_url)
        .header(reqwest::header::USER_AGENT, "opencode-provider-manager")
        .send()
        .map_err(|e| {
            AppError::Import(format!(
                "Failed to reach GitHub API ({}): {e}",
                github_short_path(owner, repo, branch, path)
            ))
        })?;

    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::Import(format!(
            "GitHub path not found: {} (branch: {branch})\n  \
             Check that the URL is correct, the repo is public, and the path exists on that branch.\n  \
             For models.dev-style providers, the URL should look like:\n  \
             https://github.com/{{owner}}/models.dev/tree/{{branch}}/providers/{{provider-name}}",
            github_short_path(owner, repo, branch, path)
        )));
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        // GitHub returns 403 for rate limiting (60 req/hr unauthenticated)
        return Err(AppError::Import(format!(
            "GitHub API rate limit hit. Unauthenticated requests are limited to 60/hour.\n  \
             Set GITHUB_TOKEN env var or wait and retry.\n  \
             Path: {}",
            github_short_path(owner, repo, branch, path)
        )));
    }
    if !status.is_success() {
        return Err(AppError::Import(format!(
            "GitHub API returned {status} for: {}",
            github_short_path(owner, repo, branch, path)
        )));
    }

    let text = response.text().map_err(|e| {
        AppError::Import(format!(
            "Failed to read GitHub response: {e}"
        ))
    })?;
    serde_json::from_str::<Vec<GithubContentEntry>>(&text).map_err(|e| {
        AppError::Import(format!(
            "Failed to parse GitHub directory listing: {e}\n  \
             The path may point to a file, not a directory. Try a raw file URL instead."
        ))
    })
}

fn github_short_path(owner: &str, repo: &str, branch: &str, path: &str) -> String {
    format!("{owner}/{repo}/{branch}/{path}")
}

#[derive(Debug, Deserialize)]
struct GithubContentEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
    download_url: Option<String>,
}

/// Summary of an import payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub provider_count: usize,
    pub model_count: usize,
    pub provider_ids: Vec<String>,
}

impl ImportSummary {
    pub fn from_config(config: &OpenCodeConfig) -> Self {
        let mut provider_ids = Vec::new();
        let mut model_count = 0;

        if let Some(providers) = &config.provider {
            for (provider_id, provider) in providers {
                provider_ids.push(provider_id.clone());
                model_count += provider.models.as_ref().map(HashMap::len).unwrap_or(0);
            }
        }
        provider_ids.sort();

        Self {
            provider_count: provider_ids.len(),
            model_count,
            provider_ids,
        }
    }
}

/// How to handle conflicts during import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMergeMode {
    /// Replace the entire config at the target layer.
    Replace,
    /// Deep merge the imported config into the existing config.
    Merge,
}

/// What scope to export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportScope {
    /// Export the merged result.
    Merged,
    /// Export only the global config.
    Global,
    /// Export only the project config.
    Project,
    /// Export only the custom config (from --config / OPENCODE_CONFIG).
    Custom,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, tempdir};

    #[test]
    fn test_export_merged_config() {
        let state = AppState::new().unwrap();
        let temp_file = NamedTempFile::new().unwrap();
        export_config(&state, temp_file.path(), ExportScope::Merged).unwrap();

        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("{"));
    }

    #[test]
    fn test_parse_full_json_config_preserves_modalities() {
        let config = parse_import_snippet(
            r#"{
              "provider": {
                "volcengine-plan": {
                  "npm": "@ai-sdk/openai-compatible",
                  "models": {
                    "glm-5.1": {
                      "name": "glm-5.1",
                      "limit": { "context": 200000, "output": 4096 },
                      "modalities": { "input": ["text"], "output": ["text"] }
                    }
                  }
                }
              }
            }"#,
            None,
            Some("test"),
        )
        .unwrap();

        let model = config.provider.unwrap()["volcengine-plan"]
            .models
            .as_ref()
            .unwrap()["glm-5.1"]
            .clone();
        assert_eq!(model.limit.unwrap().context, Some(200000));
        assert!(model.extra.contains_key("modalities"));
    }

    #[test]
    fn test_parse_provider_fragment_with_hint() {
        let config = parse_import_snippet(
            r#"{
              "npm": "@ai-sdk/openai-compatible",
              "name": "Volcano Engine",
              "options": { "baseURL": "https://example.com/v1" }
            }"#,
            Some("volcengine-plan"),
            Some("fragment"),
        )
        .unwrap();

        assert!(config.provider.unwrap().contains_key("volcengine-plan"));
    }

    #[test]
    fn test_parse_models_dev_directory() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("provider.toml"),
            r#"
name = "Xiaomi Token Plan (China)"
env = ["XIAOMI_API_KEY"]
npm = "@ai-sdk/openai-compatible"
api = "https://token-plan-cn.xiaomimimo.com/v1"
doc = "https://platform.xiaomimimo.com/#/docs"
"#,
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("models")).unwrap();
        std::fs::write(
            dir.path().join("models").join("mimo-v2-pro.toml"),
            r#"
name = "MiMo-V2-Pro"
family = "mimo"

[limit]
context = 1_000_000
output = 128_000

[modalities]
input = ["text"]
output = ["text"]
"#,
        )
        .unwrap();

        let config = parse_import_path(dir.path(), Some("xiaomi-token-plan-cn")).unwrap();
        let provider = &config.provider.unwrap()["xiaomi-token-plan-cn"];
        assert_eq!(provider.npm.as_deref(), Some("@ai-sdk/openai-compatible"));
        assert_eq!(
            provider
                .options
                .as_ref()
                .unwrap()
                .get("apiKey")
                .and_then(Value::as_str),
            Some("{env:XIAOMI_API_KEY}")
        );
        assert!(
            provider
                .models
                .as_ref()
                .unwrap()
                .contains_key("mimo-v2-pro")
        );
    }
}
