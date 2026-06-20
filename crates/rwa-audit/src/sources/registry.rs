use std::sync::LazyLock;

use anyhow::{bail, Context, Result};

use super::adapter::SourceAdapter;
use super::adapters::{
    CoinGeckoAdapter, EthplorerAdapter, GeckoTerminalAdapter, JupiterAdapter, ParaSwapAdapter,
    PublicNodeRpcAdapter, YahooFinanceAdapter,
};
use super::cache::ResponseCache;
use super::capability::PriceOracle;
use super::profile::{load_sources_yaml, SourceProfile, SourcesCacheConfig, SourcesConfig};
use super::types::SourceId;

pub struct SourceRegistry {
    profiles: std::collections::HashMap<SourceId, SourceProfile>,
    cache: SourcesCacheConfig,
}

static ETHPLORER: LazyLock<EthplorerAdapter> = LazyLock::new(EthplorerAdapter::default);

impl SourceRegistry {
    pub fn load_default() -> Result<Self> {
        let path = crate::config::config_dir().join("sources.yaml");
        let config = load_sources_yaml(&path)
            .with_context(|| format!("load sources registry from {}", path.display()))?;
        Ok(Self::from_config(config))
    }

    pub fn from_config(config: SourcesConfig) -> Self {
        Self {
            profiles: config.profiles,
            cache: config.cache,
        }
    }

    #[cfg(test)]
    pub fn from_profiles(profiles: std::collections::HashMap<SourceId, SourceProfile>) -> Self {
        Self {
            profiles,
            cache: SourcesCacheConfig::default(),
        }
    }

    pub fn profile(&self, id: SourceId) -> Option<&SourceProfile> {
        self.profiles.get(&id)
    }

    pub fn require_profile(&self, id: SourceId) -> Result<&SourceProfile> {
        self.profiles.get(&id).with_context(|| {
            format!(
                "source `{}` is not configured in config/sources.yaml",
                id.as_str()
            )
        })
    }

    pub fn profiles(&self) -> &std::collections::HashMap<SourceId, SourceProfile> {
        &self.profiles
    }

    pub fn cache_config(&self) -> &SourcesCacheConfig {
        &self.cache
    }

    pub fn build_cache(&self) -> ResponseCache {
        if !self.cache.enabled {
            ResponseCache::disabled()
        } else {
            ResponseCache::new(crate::config::repo_root().join(&self.cache.root))
        }
    }

    pub fn rpc_url(&self, chain: &str) -> Result<String> {
        let profile = self.require_profile(SourceId::PublicNodeRpc)?;
        Ok(profile.rpc_url(chain)?.to_string())
    }

    pub fn resolve_adapter(&self, id: SourceId) -> Result<&'static dyn SourceAdapter> {
        self.require_profile(id)?;
        resolve_adapter_impl(id)
    }

    pub fn price_oracle(&self, id: SourceId) -> Result<&'static dyn PriceOracle> {
        self.require_profile(id)?;
        resolve_price_oracle_impl(id)
    }
}

fn resolve_adapter_impl(id: SourceId) -> Result<&'static dyn SourceAdapter> {
    static RPC: PublicNodeRpcAdapter = PublicNodeRpcAdapter;
    static COINGECKO: CoinGeckoAdapter = CoinGeckoAdapter;
    static GECKO: GeckoTerminalAdapter = GeckoTerminalAdapter;
    static JUPITER: JupiterAdapter = JupiterAdapter;
    static PARASWAP: ParaSwapAdapter = ParaSwapAdapter;
    static YAHOO: YahooFinanceAdapter = YahooFinanceAdapter;

    let adapter: &dyn SourceAdapter = match id {
        SourceId::PublicNodeRpc => &RPC,
        SourceId::CoinGecko => &COINGECKO,
        SourceId::Ethplorer => &*ETHPLORER,
        SourceId::GeckoTerminal => &GECKO,
        SourceId::Jupiter => &JUPITER,
        SourceId::ParaSwap => &PARASWAP,
        SourceId::YahooFinance => &YAHOO,
        SourceId::ManualImport | SourceId::RwaXyz => {
            bail!("{} has no JSON SourceAdapter", id.as_str())
        }
    };
    Ok(adapter)
}

fn resolve_price_oracle_impl(id: SourceId) -> Result<&'static dyn PriceOracle> {
    static COINGECKO: CoinGeckoAdapter = CoinGeckoAdapter;

    let oracle: &dyn PriceOracle = match id {
        SourceId::CoinGecko => &COINGECKO,
        other => anyhow::bail!("{} does not implement PriceOracle", other.as_str()),
    };
    Ok(oracle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_adapters() {
        let reg = SourceRegistry::load_default().unwrap();
        assert!(reg.resolve_adapter(SourceId::CoinGecko).is_ok());
        assert!(reg.resolve_adapter(SourceId::GeckoTerminal).is_ok());
        assert!(reg.resolve_adapter(SourceId::RwaXyz).is_err());
    }

    #[test]
    fn build_cache_uses_yaml_root() {
        let reg = SourceRegistry::load_default().unwrap();
        let cache = reg.build_cache();
        assert!(cache.root().ends_with("cache"));
    }

    #[test]
    fn resolve_adapter_requires_profile_entry() {
        let reg = SourceRegistry::from_profiles(std::collections::HashMap::new());
        assert!(reg.resolve_adapter(SourceId::CoinGecko).is_err());
    }
}
