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
            let path = format!("/networks/{PANEL_NETWORK}/tokens/{token_address}/pools?page={page}");
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
}
