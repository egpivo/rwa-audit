use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::config_dir;
use crate::models::{ActivityAsset, RegistryAsset};

#[derive(Debug, Deserialize)]
struct RegistryFile {
    version: u32,
    assets: Vec<RegistryAssetYaml>,
}

#[derive(Debug, Deserialize)]
struct ActivityFile {
    version: u32,
    assets: Vec<ActivityAssetYaml>,
}

#[derive(Debug, Deserialize)]
struct RegistryAssetYaml {
    asset_name: String,
    symbol: String,
    category: String,
    chain: String,
    contract_address: String,
    decimals: u32,
    #[serde(default)]
    coingecko_id: Option<String>,
    #[serde(default)]
    price_usd_approx: Option<f64>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    permissioning: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ActivityAssetYaml {
    symbol: String,
    asset_name: String,
    chain: String,
    contract: String,
    decimals: u32,
    #[serde(default)]
    price_usd_approx: Option<f64>,
    #[serde(default)]
    include_in_figure: bool,
}

pub fn default_registry_path() -> std::path::PathBuf {
    config_dir().join("assets/registry_v1.yaml")
}

pub fn default_activity_path() -> std::path::PathBuf {
    config_dir().join("assets/activity_v1.yaml")
}

pub fn load_registry(path: &Path) -> Result<Vec<RegistryAsset>> {
    let file: RegistryFile = parse_yaml(path)?;
    anyhow::ensure!(
        file.version == 1,
        "unsupported registry version {}",
        file.version
    );
    let mut assets = Vec::with_capacity(file.assets.len());
    for a in file.assets {
        validate_eth_address(&a.contract_address)?;
        assets.push(RegistryAsset {
            asset_name: a.asset_name,
            symbol: a.symbol,
            category: a.category,
            chain: a.chain,
            contract_address: a.contract_address,
            decimals: a.decimals,
            coingecko_id: a.coingecko_id,
            price_usd_approx: a.price_usd_approx,
            notes: a.notes,
        });
    }
    ensure_unique_symbols(&assets.iter().map(|a| a.symbol.as_str()).collect::<Vec<_>>())?;
    Ok(assets)
}

pub fn load_activity(path: &Path) -> Result<Vec<ActivityAsset>> {
    let file: ActivityFile = parse_yaml(path)?;
    anyhow::ensure!(
        file.version == 1,
        "unsupported activity version {}",
        file.version
    );
    let mut assets = Vec::with_capacity(file.assets.len());
    for a in file.assets {
        validate_eth_address(&a.contract)?;
        assets.push(ActivityAsset {
            symbol: a.symbol,
            asset_name: a.asset_name,
            chain: a.chain,
            contract: a.contract,
            decimals: a.decimals,
            price_usd_approx: a.price_usd_approx,
            include_in_figure: a.include_in_figure,
        });
    }
    ensure_unique_symbols(&assets.iter().map(|a| a.symbol.as_str()).collect::<Vec<_>>())?;
    Ok(assets)
}

pub fn permissioning_from_yaml(symbol: &str, registry_path: &Path) -> Option<String> {
    let raw = fs::read_to_string(registry_path).ok()?;
    let file: RegistryFile = serde_yaml::from_str(&raw).ok()?;
    file.assets
        .into_iter()
        .find(|a| a.symbol == symbol)
        .and_then(|a| a.permissioning)
}

fn parse_yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_yaml::from_str(&raw).with_context(|| format!("parse yaml {}", path.display()))
}

fn validate_eth_address(addr: &str) -> Result<()> {
    anyhow::ensure!(
        addr.starts_with("0x") && addr.len() == 42,
        "invalid contract address: {addr}"
    );
    Ok(())
}

fn ensure_unique_symbols(symbols: &[&str]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for s in symbols {
        anyhow::ensure!(seen.insert(*s), "duplicate symbol: {s}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_registry_yaml_matches_expected_count() {
        let path = default_registry_path();
        if !path.exists() {
            return;
        }
        let assets = load_registry(&path).unwrap();
        assert_eq!(assets.len(), 11);
        assert!(assets.iter().any(|a| a.symbol == "BUIDL"));
    }

    #[test]
    fn loads_activity_yaml_matches_expected_count() {
        let path = default_activity_path();
        if !path.exists() {
            return;
        }
        let assets = load_activity(&path).unwrap();
        assert_eq!(assets.len(), 5);
    }

    #[test]
    fn permissioning_from_yaml_for_buidl() {
        let path = default_registry_path();
        if !path.exists() {
            return;
        }
        assert_eq!(
            permissioning_from_yaml("BUIDL", &path).as_deref(),
            Some("allowlist")
        );
    }

    #[test]
    fn rejects_duplicate_symbols_in_yaml() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-yaml-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.yaml");
        std::fs::write(
            &path,
            r#"version: 1
assets:
  - asset_name: A
    symbol: X
    category: c
    chain: Ethereum
    contract_address: "0x0000000000000000000000000000000000000001"
    decimals: 18
  - asset_name: B
    symbol: X
    category: c
    chain: Ethereum
    contract_address: "0x0000000000000000000000000000000000000002"
    decimals: 18
"#,
        )
        .unwrap();
        assert!(load_registry(&path).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
