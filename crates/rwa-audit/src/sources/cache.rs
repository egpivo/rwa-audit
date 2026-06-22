use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;

use super::types::{cache_key, sha256_hex_bytes, SourceId};

/// RPC methods whose results depend on chain head and must not be served from a stale cache.
pub fn is_volatile_rpc(method: &str, params: &Value) -> bool {
    match method {
        "eth_blockNumber" => true,
        "eth_getBlockByNumber" => params
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .is_some_and(|tag| tag == "latest"),
        "eth_call" => params
            .as_array()
            .and_then(|a| a.get(1))
            .and_then(|v| v.as_str())
            .is_some_and(|tag| tag == "latest"),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct ResponseCache {
    root: PathBuf,
    enabled: bool,
    /// When set, entries older than this duration are treated as cache misses.
    default_ttl: Option<Duration>,
    /// Skip read/write for head-dependent RPC responses (live collection).
    bypass_volatile: bool,
}

impl ResponseCache {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            enabled: true,
            default_ttl: None,
            bypass_volatile: false,
        }
    }

    pub fn disabled() -> Self {
        Self {
            root: PathBuf::new(),
            enabled: false,
            default_ttl: None,
            bypass_volatile: false,
        }
    }

    /// Live on-chain collection: never reuse head-dependent RPC, expire other entries quickly.
    pub fn live_collection() -> Self {
        Self {
            root: crate::config::cache_dir(),
            enabled: true,
            default_ttl: Some(Duration::from_secs(300)),
            bypass_volatile: true,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_ttl(mut self, ttl: Option<Duration>) -> Self {
        self.default_ttl = ttl;
        self
    }

    pub fn with_bypass_volatile(mut self, bypass: bool) -> Self {
        self.bypass_volatile = bypass;
        self
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn entry_path(&self, source: SourceId, key: &str) -> PathBuf {
        self.root
            .join("sources")
            .join(source.as_str())
            .join(format!("{key}.json"))
    }

    fn is_expired(&self, path: &std::path::Path) -> bool {
        let Some(ttl) = self.default_ttl else {
            return false;
        };
        let Ok(meta) = fs::metadata(path) else {
            return true;
        };
        let Ok(modified) = meta.modified() else {
            return true;
        };
        modified.elapsed().map(|age| age > ttl).unwrap_or(true)
    }

    pub fn get(&self, source: SourceId, key: &str) -> Option<Value> {
        self.get_inner(source, key, false)
    }

    pub fn get_rpc(
        &self,
        method: &str,
        params: &Value,
        source: SourceId,
        key: &str,
    ) -> Option<Value> {
        let volatile = self.bypass_volatile && is_volatile_rpc(method, params);
        self.get_inner(source, key, volatile)
    }

    fn get_inner(&self, source: SourceId, key: &str, volatile: bool) -> Option<Value> {
        if !self.enabled || volatile {
            return None;
        }
        let path = self.entry_path(source, key);
        if self.is_expired(&path) {
            return None;
        }
        let raw = fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn put(&self, source: SourceId, key: &str, body: &Value) -> Result<()> {
        self.put_inner(source, key, body, false)
    }

    pub fn put_rpc(
        &self,
        method: &str,
        params: &Value,
        source: SourceId,
        key: &str,
        body: &Value,
    ) -> Result<()> {
        let volatile = self.bypass_volatile && is_volatile_rpc(method, params);
        self.put_inner(source, key, body, volatile)
    }

    fn put_inner(&self, source: SourceId, key: &str, body: &Value, volatile: bool) -> Result<()> {
        if !self.enabled || volatile {
            return Ok(());
        }
        let path = self.entry_path(source, key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, serde_json::to_string_pretty(body)? + "\n")
            .with_context(|| format!("cache write {}", path.display()))?;
        Ok(())
    }

    pub fn key_from_url(url: &str, query: &[(&str, &str)]) -> String {
        let q: Vec<String> = query.iter().map(|(k, v)| format!("{k}={v}")).collect();
        cache_key(&[url, &q.join("&")])
    }

    pub fn key_from_rpc(url: &str, method: &str, params: &Value) -> String {
        let params_s = serde_json::to_string(params).unwrap_or_default();
        cache_key(&[url, method, &params_s])
    }

    pub fn body_sha256(body: &Value) -> String {
        sha256_hex_bytes(serde_json::to_string(body).unwrap_or_default().as_bytes())
    }
}

impl Default for ResponseCache {
    fn default() -> Self {
        Self::new(crate::config::cache_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn round_trip_cache_entry() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-cache-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = ResponseCache::new(dir.clone());
        let body = json!({"usd": 1.0});
        let key = "test-key";
        cache.put(SourceId::CoinGecko, key, &body).unwrap();
        let loaded = cache.get(SourceId::CoinGecko, key).unwrap();
        assert_eq!(loaded, body);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn disabled_cache_is_noop() {
        let cache = ResponseCache::disabled();
        let body = json!({"x": 1});
        cache.put(SourceId::CoinGecko, "k", &body).unwrap();
        assert!(cache.get(SourceId::CoinGecko, "k").is_none());
    }

    #[test]
    fn volatile_rpc_bypasses_cache() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-cache-volatile-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = ResponseCache::new(dir.clone()).with_bypass_volatile(true);
        let key = "block";
        let body = json!({"result": "0x1"});
        cache
            .put_rpc(
                "eth_blockNumber",
                &json!([]),
                SourceId::PublicNodeRpc,
                key,
                &body,
            )
            .unwrap();
        assert!(cache
            .get_rpc("eth_blockNumber", &json!([]), SourceId::PublicNodeRpc, key)
            .is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ttl_expires_stale_entries() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-cache-ttl-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = ResponseCache::new(dir.clone()).with_ttl(Some(Duration::from_millis(1)));
        let body = json!({"x": 1});
        cache.put(SourceId::CoinGecko, "k", &body).unwrap();
        thread::sleep(Duration::from_millis(5));
        assert!(cache.get(SourceId::CoinGecko, "k").is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_volatile_latest_block_rpc() {
        assert!(is_volatile_rpc("eth_blockNumber", &json!([])));
        assert!(is_volatile_rpc(
            "eth_getBlockByNumber",
            &json!(["latest", false])
        ));
        assert!(!is_volatile_rpc(
            "eth_getLogs",
            &json!([{"fromBlock": "0x1"}])
        ));
    }

    #[test]
    fn is_volatile_rpc_eth_call_with_latest() {
        assert!(is_volatile_rpc("eth_call", &json!([{}, "latest"])));
    }

    #[test]
    fn is_volatile_rpc_eth_call_with_block_number_is_not_volatile() {
        assert!(!is_volatile_rpc("eth_call", &json!([{}, "0x1234567"])));
    }

    #[test]
    fn is_volatile_rpc_eth_get_block_by_number_non_latest() {
        assert!(!is_volatile_rpc(
            "eth_getBlockByNumber",
            &json!(["0xabcdef", false])
        ));
    }

    #[test]
    fn is_volatile_rpc_unknown_method() {
        assert!(!is_volatile_rpc(
            "eth_getBalance",
            &json!(["0xaddr", "latest"])
        ));
    }

    #[test]
    fn live_collection_cache_has_ttl_and_bypass() {
        let cache = ResponseCache::live_collection();
        assert!(cache.is_enabled());
        assert!(cache.bypass_volatile);
        assert!(cache.default_ttl.is_some());
    }

    #[test]
    fn key_from_url_differs_by_query() {
        let k1 = ResponseCache::key_from_url("https://example.com", &[("a", "1")]);
        let k2 = ResponseCache::key_from_url("https://example.com", &[("a", "2")]);
        let k3 = ResponseCache::key_from_url("https://example.com", &[]);
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
        assert_eq!(k1.len(), 64);
    }

    #[test]
    fn body_sha256_is_deterministic_hex() {
        let b = json!({"key": "value"});
        let h1 = ResponseCache::body_sha256(&b);
        let h2 = ResponseCache::body_sha256(&b);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        assert_ne!(h1, ResponseCache::body_sha256(&json!({"key": "other"})));
    }

    #[test]
    fn get_rpc_non_volatile_returns_cached_value() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-cache-rpc-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = ResponseCache::new(dir.clone()).with_bypass_volatile(true);
        let params = json!(["0xabc", false]);
        let body = json!({"result": {"number": "0xabc"}});
        cache
            .put_rpc(
                "eth_getBlockByNumber",
                &params,
                SourceId::PublicNodeRpc,
                "k1",
                &body,
            )
            .unwrap();
        let hit = cache.get_rpc(
            "eth_getBlockByNumber",
            &params,
            SourceId::PublicNodeRpc,
            "k1",
        );
        assert_eq!(hit.unwrap(), body);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn key_from_rpc_stable() {
        let k1 = ResponseCache::key_from_rpc("https://eth.rpc", "eth_call", &json!([{}, "latest"]));
        let k2 = ResponseCache::key_from_rpc("https://eth.rpc", "eth_call", &json!([{}, "latest"]));
        assert_eq!(k1, k2);
        assert_ne!(
            k1,
            ResponseCache::key_from_rpc("https://eth.rpc", "eth_call", &json!([{}, "0x1"]))
        );
    }
}
