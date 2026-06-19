use crate::asset_config::{
    default_activity_path, default_registry_path, load_activity, load_registry,
};
use crate::models::{ActivityAsset, RegistryAsset};

pub fn registry_assets() -> Vec<RegistryAsset> {
    registry_assets_from(&default_registry_path()).expect("load config/assets/registry_v1.yaml")
}

pub fn registry_assets_from(path: &std::path::Path) -> anyhow::Result<Vec<RegistryAsset>> {
    load_registry(path)
}

pub fn activity_assets() -> Vec<ActivityAsset> {
    activity_assets_from(&default_activity_path()).expect("load config/assets/activity_v1.yaml")
}

pub fn activity_assets_from(path: &std::path::Path) -> anyhow::Result<Vec<ActivityAsset>> {
    load_activity(path)
}

pub fn detect_permissioning_from_known(symbol: &str) -> Option<String> {
    if let Some(p) = crate::asset_config::permissioning_from_yaml(symbol, &default_registry_path())
    {
        return Some(map_permissioning_label(&p));
    }
    None
}

fn map_permissioning_label(raw: &str) -> String {
    match raw {
        "allowlist" => "true".into(),
        "open" => "false".into(),
        "partial" => "partial".into(),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn detect_permissioning_from_known_symbols() {
        assert_eq!(
            detect_permissioning_from_known("BUIDL").as_deref(),
            Some("true")
        );
        assert_eq!(
            detect_permissioning_from_known("PAXG").as_deref(),
            Some("false")
        );
        assert_eq!(
            detect_permissioning_from_known("USDY").as_deref(),
            Some("partial")
        );
        assert!(detect_permissioning_from_known("UNKNOWN").is_none());
    }

    #[test]
    fn registry_assets_have_unique_symbols_and_addresses() {
        let assets = registry_assets();
        assert_eq!(assets.len(), 11);
        let symbols: HashSet<_> = assets.iter().map(|a| a.symbol.as_str()).collect();
        assert_eq!(symbols.len(), assets.len());
        for a in &assets {
            assert!(a.contract_address.starts_with("0x"));
            assert!(a.decimals <= 18);
        }
    }

    #[test]
    fn activity_assets_include_chart_subset() {
        let assets = activity_assets();
        assert_eq!(assets.len(), 5);
        assert!(assets
            .iter()
            .any(|a| a.symbol == "BENJI" && !a.include_in_figure));
        assert!(assets.iter().filter(|a| a.include_in_figure).count() >= 4);
    }
}
