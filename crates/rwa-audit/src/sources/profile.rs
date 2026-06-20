use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::types::SourceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Rpc,
    Http,
    File,
}

#[derive(Debug, Clone)]
pub struct SourceProfile {
    pub id: SourceId,
    pub kind: SourceKind,
    pub base_url: Option<String>,
    pub rpc_endpoints: HashMap<String, String>,
    pub base_path: Option<PathBuf>,
    pub env_keys: Vec<String>,
    pub rate_limit_ms: u64,
    pub default_headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SourcesCacheConfig {
    pub enabled: bool,
    /// Repo-relative cache root; entries live at `{root}/sources/{{source_id}}/`.
    pub root: PathBuf,
}

impl Default for SourcesCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            root: PathBuf::from("cache"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourcesConfig {
    pub profiles: HashMap<SourceId, SourceProfile>,
    pub cache: SourcesCacheConfig,
}

impl SourceProfile {
    pub fn http_base(&self) -> Result<&str> {
        self.base_url
            .as_deref()
            .context("http source missing base_url")
    }

    pub fn rpc_url(&self, chain: &str) -> Result<&str> {
        let key = match chain {
            "Ethereum" | "ethereum" => "ethereum",
            "Polygon" | "polygon" => "polygon",
            other => other,
        };
        self.rpc_endpoints
            .get(key)
            .map(String::as_str)
            .with_context(|| format!("no RPC endpoint for chain {chain} on {}", self.id.as_str()))
    }
}

#[derive(Debug, Deserialize)]
struct SourcesFile {
    sources: HashMap<String, RawSourceEntry>,
    #[serde(default)]
    cache: RawCacheConfig,
}

#[derive(Debug, Deserialize, Default)]
struct RawCacheConfig {
    #[serde(default = "default_cache_enabled")]
    enabled: bool,
    #[serde(default = "default_cache_root")]
    root: String,
}

fn default_cache_enabled() -> bool {
    true
}

fn default_cache_root() -> String {
    "cache".into()
}

#[derive(Debug, Deserialize)]
struct RawSourceEntry {
    kind: String,
    base_url: Option<String>,
    base_path: Option<String>,
    ethereum: Option<String>,
    polygon: Option<String>,
    #[serde(default)]
    env: Vec<String>,
    #[serde(default)]
    rate_limit_ms: Option<u64>,
    #[serde(default)]
    default_headers: HashMap<String, String>,
}

pub fn parse_sources_yaml(text: &str) -> Result<SourcesConfig> {
    let file: SourcesFile = serde_yaml::from_str(text).context("parse config/sources.yaml")?;
    let mut profiles = HashMap::new();
    for (name, entry) in file.sources {
        let id = source_id_from_key(&name)
            .with_context(|| format!("unknown source id in sources.yaml: {name}"))?;
        profiles.insert(id, raw_entry_to_profile(id, entry)?);
    }
    Ok(SourcesConfig {
        profiles,
        cache: SourcesCacheConfig {
            enabled: file.cache.enabled,
            root: PathBuf::from(file.cache.root),
        },
    })
}

fn source_id_from_key(key: &str) -> Result<SourceId> {
    match key {
        "publicnode_rpc" => Ok(SourceId::PublicNodeRpc),
        "coingecko" => Ok(SourceId::CoinGecko),
        "ethplorer" => Ok(SourceId::Ethplorer),
        "geckoterminal" => Ok(SourceId::GeckoTerminal),
        "paraswap" => Ok(SourceId::ParaSwap),
        "jupiter" => Ok(SourceId::Jupiter),
        "yahoo_finance" => Ok(SourceId::YahooFinance),
        "rwa_xyz" => Ok(SourceId::RwaXyz),
        "manual_import" => Ok(SourceId::ManualImport),
        other => anyhow::bail!("unknown source: {other}"),
    }
}

fn parse_source_kind(kind: &str) -> Result<SourceKind> {
    match kind {
        "rpc" => Ok(SourceKind::Rpc),
        "http" => Ok(SourceKind::Http),
        "file" => Ok(SourceKind::File),
        other => anyhow::bail!("unknown source kind in sources.yaml: {other}"),
    }
}

fn raw_entry_to_profile(id: SourceId, entry: RawSourceEntry) -> Result<SourceProfile> {
    let kind = parse_source_kind(&entry.kind)?;

    let mut rpc_endpoints = HashMap::new();
    if let Some(url) = entry.ethereum {
        rpc_endpoints.insert("ethereum".into(), url);
    }
    if let Some(url) = entry.polygon {
        rpc_endpoints.insert("polygon".into(), url);
    }

    let base_path = entry.base_path.map(PathBuf::from);

    Ok(SourceProfile {
        id,
        kind,
        base_url: entry.base_url,
        rpc_endpoints,
        base_path,
        env_keys: entry.env,
        rate_limit_ms: entry
            .rate_limit_ms
            .unwrap_or_else(|| default_rate_limit_ms(id)),
        default_headers: entry.default_headers,
    })
}

fn default_rate_limit_ms(id: SourceId) -> u64 {
    match id {
        SourceId::PublicNodeRpc => 150,
        SourceId::GeckoTerminal => 3_500,
        SourceId::ParaSwap => 1_000,
        SourceId::CoinGecko | SourceId::Ethplorer | SourceId::Jupiter | SourceId::YahooFinance => {
            500
        }
        SourceId::RwaXyz => 1_000,
        SourceId::ManualImport => 0,
    }
}

pub fn load_sources_yaml(path: &Path) -> Result<SourcesConfig> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse_sources_yaml(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_sources_yaml() {
        let path = crate::config::config_dir().join("sources.yaml");
        if !path.exists() {
            return;
        }
        let config = load_sources_yaml(&path).unwrap();
        let gecko = config.profiles.get(&SourceId::GeckoTerminal).unwrap();
        assert!(gecko.base_url.as_ref().unwrap().contains("geckoterminal"));
        let rpc = config.profiles.get(&SourceId::PublicNodeRpc).unwrap();
        assert!(rpc.rpc_endpoints.contains_key("ethereum"));
        assert!(config.cache.enabled);
        assert_eq!(config.cache.root, PathBuf::from("cache"));
    }

    #[test]
    fn rejects_unknown_source_kind() {
        let yaml = r#"
version: 1
sources:
  broken:
    kind: htttp
    base_url: https://example.com
cache:
  enabled: true
  root: cache
"#;
        assert!(parse_sources_yaml(yaml).is_err());
    }
}
