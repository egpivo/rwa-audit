use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate};
use reqwest::blocking::Client;
use serde_json::Value;

use crate::flow::config::{GECKO_BASE, GECKO_SLEEP_MS, MAX_POOLS_PER_TOKEN, PANEL_NETWORK};

pub struct GeckoClient {
    http: Client,
}

impl GeckoClient {
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .user_agent("rwa-audit/0.1")
            .timeout(Duration::from_secs(45))
            .build()?;
        Ok(Self { http })
    }

    fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{GECKO_BASE}{path}");
        for attempt in 0..5u32 {
            thread::sleep(Duration::from_millis(GECKO_SLEEP_MS));
            let resp = self
                .http
                .get(&url)
                .header("accept", "application/json")
                .send()
                .context(format!("GeckoTerminal GET {url}"))?;
            if resp.status().as_u16() == 429 {
                let wait = 5 * 2u64.pow(attempt);
                eprintln!("    Rate limited, sleeping {wait}s...");
                thread::sleep(Duration::from_secs(wait));
                continue;
            }
            return resp.error_for_status()?.json().context("gecko json");
        }
        anyhow::bail!("GeckoTerminal rate limit exceeded for {url}")
    }

    pub fn token_pools(&self, token_address: &str) -> Result<Vec<PoolMeta>> {
        let mut pools = Vec::new();
        let mut page = 1u32;
        loop {
            let path =
                format!("/networks/{PANEL_NETWORK}/tokens/{token_address}/pools?page={page}");
            let body = self.get(&path)?;
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

    pub fn pool_daily_ohlcv(&self, pool_address: &str, limit: u32) -> Result<Vec<DailyOhlcv>> {
        let path = format!(
            "/networks/{PANEL_NETWORK}/pools/{pool_address}/ohlcv/day?aggregate=1&limit={limit}"
        );
        let body = self.get(&path)?;
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
}

#[derive(Debug, Clone)]
pub struct PoolMeta {
    pub address: String,
    pub name: String,
    pub reserve_usd: f64,
    pub volume_h24_usd: f64,
}

impl PoolMeta {
    fn from_gecko(item: &Value) -> Option<Self> {
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

// --- Solana xStock pool search aggregate (exchange-layer publish metrics) ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolPoolAggregate {
    pub symbol: String,
    pub pool_count: usize,
    pub total_tvl_usd: f64,
    pub total_24h_vol_usd: f64,
    pub top_pool_vol_share: Option<f64>,
    pub source_url: String,
}

pub fn fetch_solana_symbol_pool_aggregate(
    client: &Client,
    symbol: &str,
) -> Result<SymbolPoolAggregate> {
    let path = format!("/search/pools?query={symbol}&network=solana");
    let url = format!("{GECKO_BASE}{path}");
    for attempt in 0..5u32 {
        thread::sleep(Duration::from_millis(GECKO_SLEEP_MS));
        let resp = client
            .get(&url)
            .header("accept", "application/json")
            .header("User-Agent", "rwa-audit/0.1")
            .send()
            .context("gecko search pools")?;
        if resp.status().as_u16() == 429 {
            thread::sleep(Duration::from_secs(5 * 2u64.pow(attempt)));
            continue;
        }
        let body: Value = resp.error_for_status()?.json()?;
        return Ok(aggregate_solana_search(symbol, &body, &url));
    }
    anyhow::bail!("GeckoTerminal rate limit for {symbol}")
}

pub fn aggregate_solana_search(symbol: &str, body: &Value, url: &str) -> SymbolPoolAggregate {
    use crate::flow::config::OUTLIER_TVL_USD;

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
        source_url: url.to_string(),
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
    fn solana_aggregate_excludes_outlier_tvl() {
        let body = json!({
            "data": [
                {"attributes": {"reserve_in_usd": "100000", "volume_usd": {"h24": "5000"}}},
                {"attributes": {"reserve_in_usd": "60000000", "volume_usd": {"h24": "999999"}}},
                {"attributes": {"reserve_in_usd": "24000", "volume_usd": {"h24": "30000"}}}
            ]
        });
        let agg = aggregate_solana_search("AAPLx", &body, "http://example");
        assert_eq!(agg.pool_count, 2);
        assert!((agg.total_tvl_usd - 124_000.0).abs() < f64::EPSILON);
        assert!((agg.total_24h_vol_usd - 35_000.0).abs() < f64::EPSILON);
    }
}
