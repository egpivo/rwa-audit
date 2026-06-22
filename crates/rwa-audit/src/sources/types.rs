use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    #[serde(rename = "publicnode_rpc")]
    PublicNodeRpc,
    #[serde(rename = "coingecko")]
    CoinGecko,
    Ethplorer,
    #[serde(rename = "geckoterminal")]
    GeckoTerminal,
    ParaSwap,
    Jupiter,
    #[serde(rename = "yahoo_finance")]
    YahooFinance,
    #[serde(rename = "manual_import")]
    ManualImport,
    #[serde(rename = "rwa_xyz")]
    RwaXyz,
}

impl SourceId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PublicNodeRpc => "publicnode_rpc",
            Self::CoinGecko => "coingecko",
            Self::Ethplorer => "ethplorer",
            Self::GeckoTerminal => "geckoterminal",
            Self::ParaSwap => "paraswap",
            Self::Jupiter => "jupiter",
            Self::YahooFinance => "yahoo_finance",
            Self::ManualImport => "manual_import",
            Self::RwaXyz => "rwa_xyz",
        }
    }
}

#[derive(Debug, Clone)]
pub enum SourceRequest {
    Rpc {
        url: String,
        method: String,
        params: Value,
    },
    HttpGet {
        url: String,
        query: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Provenance {
    pub source: String,
    pub fetched_at: String,
    pub request_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_sha256: Option<String>,
    pub live: bool,
}

#[derive(Debug, Clone)]
pub struct SourceResponse {
    pub body: Value,
    pub provenance: Provenance,
}

impl Provenance {
    pub fn new(source: SourceId, request_url: impl Into<String>, live: bool) -> Self {
        Self {
            source: source.as_str().into(),
            fetched_at: chrono::Utc::now().to_rfc3339(),
            request_url: request_url.into(),
            response_sha256: None,
            live,
        }
    }

    pub fn with_sha256(mut self, hex: String) -> Self {
        self.response_sha256 = Some(hex);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceEnvelope<T: Serialize> {
    pub provenance: Provenance,
    pub data: T,
}

pub fn cache_key(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let joined = parts.join("|");
    let digest = Sha256::digest(joined.as_bytes());
    format!("{:x}", digest)
}

pub fn sha256_hex_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    format!("{:x}", digest)
}

pub fn repo_fixture(path: &str) -> PathBuf {
    crate::config::repo_root().join("data/fixtures").join(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable() {
        let a = cache_key(&["coingecko", "buidl", "usd"]);
        let b = cache_key(&["coingecko", "buidl", "usd"]);
        assert_eq!(a, b);
        assert_ne!(a, cache_key(&["coingecko", "buidl", "eur"]));
    }

    #[test]
    fn cache_key_is_64_hex_chars() {
        let k = cache_key(&["a", "b"]);
        assert_eq!(k.len(), 64);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn source_id_serializes_snake_case() {
        let json = serde_json::to_string(&SourceId::CoinGecko).unwrap();
        assert_eq!(json, "\"coingecko\"");
    }

    #[test]
    fn source_id_as_str_all_variants() {
        assert_eq!(SourceId::PublicNodeRpc.as_str(), "publicnode_rpc");
        assert_eq!(SourceId::CoinGecko.as_str(), "coingecko");
        assert_eq!(SourceId::Ethplorer.as_str(), "ethplorer");
        assert_eq!(SourceId::GeckoTerminal.as_str(), "geckoterminal");
        assert_eq!(SourceId::ParaSwap.as_str(), "paraswap");
        assert_eq!(SourceId::Jupiter.as_str(), "jupiter");
        assert_eq!(SourceId::YahooFinance.as_str(), "yahoo_finance");
        assert_eq!(SourceId::ManualImport.as_str(), "manual_import");
        assert_eq!(SourceId::RwaXyz.as_str(), "rwa_xyz");
    }

    #[test]
    fn sha256_hex_bytes_returns_64_char_hex() {
        let h = sha256_hex_bytes(b"hello");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(h, sha256_hex_bytes(b"world"));
    }

    #[test]
    fn provenance_new_and_with_sha256() {
        let p = Provenance::new(SourceId::Jupiter, "https://test.example", true)
            .with_sha256("abc123".into());
        assert_eq!(p.source, "jupiter");
        assert_eq!(p.request_url, "https://test.example");
        assert!(p.live);
        assert_eq!(p.response_sha256.as_deref(), Some("abc123"));

        let p2 = Provenance::new(SourceId::CoinGecko, "https://u", false);
        assert!(!p2.live);
        assert!(p2.response_sha256.is_none());
    }
}
