use anyhow::{bail, Context, Result};
use chrono::{DateTime, NaiveDate};
use serde_json::Value;

use super::super::adapter::SourceAdapter;
use super::super::context::SourceContext;
use super::super::fetch::http_get_gecko;
use super::super::types::{Provenance, SourceId, SourceRequest, SourceResponse};

const MAX_POOLS_PER_TOKEN: usize = 25;
const OUTLIER_TVL_USD: f64 = 50_000_000.0;

pub struct GeckoTerminalAdapter;

impl SourceAdapter for GeckoTerminalAdapter {
    fn id(&self) -> SourceId {
        SourceId::GeckoTerminal
    }

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            bail!("GeckoTerminalAdapter expects HttpGet request");
        };
        http_get_gecko(self, ctx, &url, &query)
    }
}

impl GeckoTerminalAdapter {
    fn base_url(ctx: &SourceContext) -> Result<String> {
        ctx.http_base_url(SourceId::GeckoTerminal)
    }

    fn get_path(ctx: &SourceContext, path: &str) -> Result<Value> {
        let base = Self::base_url(ctx)?;
        let url = format!("{base}{path}");
        Ok(http_get_gecko(&GeckoTerminalAdapter, ctx, &url, &[])?.body)
    }

    pub fn token_pools(
        &self,
        ctx: &SourceContext,
        network: &str,
        token_address: &str,
    ) -> Result<Vec<PoolMeta>> {
        let mut pools = Vec::new();
        let mut page = 1u32;
        loop {
            let path = format!("/networks/{network}/tokens/{token_address}/pools?page={page}");
            let body = Self::get_path(ctx, &path)?;
            let batch = body
                .get("data")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            if batch.is_empty() {
                break;
            }
            for item in batch {
                if let Some(meta) = PoolMeta::from_gecko(&item) {
                    pools.push(meta);
                }
            }
            page += 1;
            if page > 2 {
                break;
            }
        }
        pools.sort_by(|a, b| {
            b.volume_h24_usd
                .partial_cmp(&a.volume_h24_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        pools.truncate(MAX_POOLS_PER_TOKEN);
        Ok(pools)
    }

    pub fn pool_daily_ohlcv(
        &self,
        ctx: &SourceContext,
        network: &str,
        pool_address: &str,
        limit: u32,
    ) -> Result<Vec<DailyOhlcv>> {
        let path =
            format!("/networks/{network}/pools/{pool_address}/ohlcv/day?aggregate=1&limit={limit}");
        let body = Self::get_path(ctx, &path)?;
        let list = body
            .pointer("/data/attributes/ohlcv_list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut rows = Vec::new();
        for row in list {
            let arr = row.as_array().context("ohlcv row array")?;
            if arr.len() < 6 {
                continue;
            }
            let ts = arr[0].as_i64().context("timestamp")?;
            let volume = arr[5].as_f64().unwrap_or(0.0);
            let date = DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.date_naive())
                .context("invalid ohlcv timestamp")?;
            rows.push(DailyOhlcv {
                date,
                volume_usd: volume,
            });
        }
        Ok(rows)
    }

    pub fn solana_symbol_pool_aggregate(
        &self,
        ctx: &SourceContext,
        symbol: &str,
    ) -> Result<SymbolPoolAggregate> {
        let base = Self::base_url(ctx)?;
        let path = format!("/search/pools?query={symbol}&network=solana");
        let url = format!("{base}{path}");
        let resp = http_get_gecko(self, ctx, &url, &[])?;
        Ok(aggregate_solana_search(symbol, &resp.body, resp.provenance))
    }
}

#[derive(Debug, Clone)]
pub struct PoolMeta {
    pub address: String,
    pub name: String,
    pub reserve_usd: f64,
    pub volume_h24_usd: f64,
}

impl PoolMeta {
    pub fn from_gecko(item: &Value) -> Option<Self> {
        let attrs = item.get("attributes")?;
        Some(PoolMeta {
            address: attrs.get("address")?.as_str()?.to_lowercase(),
            name: attrs.get("name")?.as_str().unwrap_or("").to_string(),
            reserve_usd: attrs
                .get("reserve_in_usd")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            volume_h24_usd: attrs
                .pointer("/volume_usd/h24")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DailyOhlcv {
    pub date: NaiveDate,
    pub volume_usd: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolPoolAggregate {
    pub symbol: String,
    pub pool_count: usize,
    pub total_tvl_usd: f64,
    pub total_24h_vol_usd: f64,
    pub top_pool_vol_share: Option<f64>,
    pub source_url: String,
    #[serde(skip)]
    pub provenance: Option<Provenance>,
}

pub fn aggregate_solana_search(
    symbol: &str,
    body: &Value,
    provenance: Provenance,
) -> SymbolPoolAggregate {
    let pools = body
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    let mut filtered = Vec::new();
    for p in pools {
        let a = &p["attributes"];
        let tvl = a
            .get("reserve_in_usd")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| a.get("reserve_in_usd").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        if tvl > OUTLIER_TVL_USD {
            continue;
        }
        let vol = a
            .pointer("/volume_usd/h24")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| a.pointer("/volume_usd/h24").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        filtered.push((tvl, vol));
    }
    let total_tvl: f64 = filtered.iter().map(|(t, _)| t).sum();
    let total_vol: f64 = filtered.iter().map(|(_, v)| v).sum();
    let top_vol = filtered
        .iter()
        .map(|(_, v)| v)
        .copied()
        .fold(0.0f64, f64::max);
    SymbolPoolAggregate {
        symbol: symbol.to_string(),
        pool_count: filtered.len(),
        total_tvl_usd: (total_tvl * 100.0).round() / 100.0,
        total_24h_vol_usd: (total_vol * 100.0).round() / 100.0,
        top_pool_vol_share: if total_vol > 0.0 {
            Some(((top_vol / total_vol) * 10000.0).round() / 10000.0)
        } else {
            None
        },
        source_url: provenance.request_url.clone(),
        provenance: Some(provenance),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pool_meta_parses_gecko_item() {
        let item = json!({
            "attributes": {
                "address": "0xabc",
                "name": "PAXG / USDC",
                "reserve_in_usd": "1000.5",
                "volume_usd": { "h24": "500.25" }
            }
        });
        let meta = PoolMeta::from_gecko(&item).unwrap();
        assert_eq!(meta.address, "0xabc");
        assert!((meta.reserve_usd - 1000.5).abs() < f64::EPSILON);
    }

    #[test]
    fn pool_meta_returns_none_without_attributes() {
        let item = json!({"id": "abc", "type": "pool"});
        assert!(PoolMeta::from_gecko(&item).is_none());
    }

    #[test]
    fn pool_meta_volume_defaults_to_zero_when_absent() {
        let item = json!({
            "attributes": {
                "address": "0xdef",
                "name": "TOKEN/USDC",
                "reserve_in_usd": "5000"
            }
        });
        let meta = PoolMeta::from_gecko(&item).unwrap();
        assert_eq!(meta.volume_h24_usd, 0.0);
        assert_eq!(meta.reserve_usd, 5000.0);
    }

    #[test]
    fn solana_aggregate_excludes_outlier_tvl() {
        let body = json!({
            "data": [
                {"attributes": {"reserve_in_usd": "100000", "volume_usd": {"h24": "5000"}}},
                {"attributes": {"reserve_in_usd": "60000000", "volume_usd": {"h24": "999999"}}},
                {"attributes": {"reserve_in_usd": "24000", "volume_usd": {"h24": "30000"}}}
            ]
        });
        let agg = aggregate_solana_search(
            "AAPLx",
            &body,
            Provenance::new(SourceId::GeckoTerminal, "http://example", false),
        );
        assert_eq!(agg.pool_count, 2);
        assert!((agg.total_tvl_usd - 124_000.0).abs() < f64::EPSILON);
        assert!((agg.total_24h_vol_usd - 35_000.0).abs() < f64::EPSILON);
        assert!(agg.provenance.is_some());
        assert!(serde_json::to_value(&agg)
            .unwrap()
            .get("provenance")
            .is_none());
    }

    #[test]
    fn aggregate_returns_empty_when_no_data() {
        let body = json!({"data": []});
        let agg = aggregate_solana_search(
            "AAPLx",
            &body,
            Provenance::new(SourceId::GeckoTerminal, "http://example", false),
        );
        assert_eq!(agg.pool_count, 0);
        assert_eq!(agg.total_tvl_usd, 0.0);
        assert_eq!(agg.total_24h_vol_usd, 0.0);
        assert!(agg.top_pool_vol_share.is_none());
    }

    #[test]
    fn aggregate_all_outliers_gives_empty_result() {
        let body = json!({
            "data": [
                {"attributes": {"reserve_in_usd": "100000000", "volume_usd": {"h24": "500000"}}}
            ]
        });
        let agg = aggregate_solana_search(
            "SPYx",
            &body,
            Provenance::new(SourceId::GeckoTerminal, "http://example", false),
        );
        assert_eq!(agg.pool_count, 0);
        assert!(agg.top_pool_vol_share.is_none());
    }

    #[test]
    fn aggregate_zero_volume_has_no_top_share() {
        let body = json!({
            "data": [
                {"attributes": {"reserve_in_usd": "10000", "volume_usd": {"h24": "0"}}}
            ]
        });
        let agg = aggregate_solana_search(
            "AAPLx",
            &body,
            Provenance::new(SourceId::GeckoTerminal, "http://example", false),
        );
        assert_eq!(agg.pool_count, 1);
        assert_eq!(agg.total_24h_vol_usd, 0.0);
        assert!(agg.top_pool_vol_share.is_none());
    }

    use crate::sources::cache::ResponseCache;
    use crate::sources::profile::{SourceKind, SourceProfile};
    use crate::sources::registry::SourceRegistry;
    use crate::sources::test_support::MockHttpServer;
    use std::collections::HashMap;

    fn context_for_gecko(base_url: &str) -> crate::sources::context::SourceContext {
        let profile = SourceProfile {
            id: SourceId::GeckoTerminal,
            kind: SourceKind::Http,
            base_url: Some(base_url.to_string()),
            rpc_endpoints: HashMap::new(),
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::new(),
        };
        crate::sources::context::SourceContext::with_registry(SourceRegistry::from_profiles(
            HashMap::from([(SourceId::GeckoTerminal, profile)]),
        ))
        .unwrap()
        .with_cache(ResponseCache::disabled())
    }

    #[test]
    fn pool_daily_ohlcv_parses_list() {
        let body = r#"{"data":{"attributes":{"ohlcv_list":[[1749600000,180.5,185.0,179.0,183.0,1234.56]]}}}"#;
        let server = MockHttpServer::spawn("200 OK", "application/json", body);
        let ctx = context_for_gecko(&server.url);
        let adapter = GeckoTerminalAdapter;
        let rows = adapter
            .pool_daily_ohlcv(&ctx, "ethereum", "0xpool", 1)
            .unwrap();
        server.request();
        assert_eq!(rows.len(), 1);
        assert!((rows[0].volume_usd - 1234.56).abs() < 0.001);
    }

    #[test]
    fn pool_daily_ohlcv_skips_short_rows() {
        // Row with only 4 elements (< 6) should be skipped
        let body = r#"{"data":{"attributes":{"ohlcv_list":[[1749600000,180.5,185.0,179.0]]}}}"#;
        let server = MockHttpServer::spawn("200 OK", "application/json", body);
        let ctx = context_for_gecko(&server.url);
        let adapter = GeckoTerminalAdapter;
        let rows = adapter
            .pool_daily_ohlcv(&ctx, "ethereum", "0xpool", 1)
            .unwrap();
        server.request();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn token_pools_empty_first_page_returns_empty() {
        // When the first page returns empty data array, the loop breaks immediately
        let body = r#"{"data":[]}"#;
        let server = MockHttpServer::spawn("200 OK", "application/json", body);
        let ctx = context_for_gecko(&server.url);
        let adapter = GeckoTerminalAdapter;
        let pools = adapter.token_pools(&ctx, "solana", "0xtoken123").unwrap();
        server.request();
        assert!(pools.is_empty());
    }

    #[test]
    fn adapter_rejects_rpc_request() {
        let ctx = context_for_gecko("http://example.test");
        let err = GeckoTerminalAdapter
            .fetch(
                &ctx,
                crate::sources::types::SourceRequest::Rpc {
                    url: "http://example.test".into(),
                    method: "eth_blockNumber".into(),
                    params: serde_json::json!([]),
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("expects HttpGet"));
    }

    #[test]
    fn solana_symbol_pool_aggregate_from_mock() {
        let body =
            r#"{"data":[{"attributes":{"reserve_in_usd":"5000","volume_usd":{"h24":"3000"}}}]}"#;
        let server = MockHttpServer::spawn("200 OK", "application/json", body);
        let ctx = context_for_gecko(&server.url);
        let adapter = GeckoTerminalAdapter;
        let agg = adapter.solana_symbol_pool_aggregate(&ctx, "AAPLx").unwrap();
        server.request();
        assert_eq!(agg.pool_count, 1);
        assert!((agg.total_tvl_usd - 5000.0).abs() < 0.01);
    }
}
