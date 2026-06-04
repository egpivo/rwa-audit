use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::Client;
use serde_json::{json, Value};

use crate::config::{
    rpc_for_chain, COINGECKO_BASE, SLEEP_BETWEEN_API_MS, SLEEP_BETWEEN_RPC_MS,
    TRANSFER_TOPIC,
};
use crate::models::{RpcResponse, TransferLog};

pub struct HttpClient {
    inner: Client,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let inner = Client::builder()
            .user_agent("rwa-audit/0.1")
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { inner })
    }

    pub fn http_get(&self, url: &str, params: &[(&str, &str)], retries: u32) -> Result<Option<Value>> {
        for attempt in 0..retries {
            let mut req = self.inner.get(url);
            if !params.is_empty() {
                req = req.query(params);
            }
            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.as_u16() == 429 {
                        let wait = 3 * 2u64.pow(attempt);
                        eprintln!("    Rate limited, sleeping {wait}s...");
                        thread::sleep(Duration::from_secs(wait));
                        continue;
                    }
                    if !status.is_success() {
                        eprintln!("    HTTP {} for {}", status, &url[..url.len().min(80)]);
                        return Ok(None);
                    }
                    return Ok(Some(resp.json()?));
                }
                Err(e) => {
                    eprintln!("    Request error ({}/{}): {e}", attempt + 1, retries);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Ok(None)
    }

    pub fn rpc_call(
        &self,
        rpc_url: &str,
        method: &str,
        params: Value,
        retries: u32,
    ) -> Result<Option<RpcResponse>> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        for attempt in 0..retries {
            match self
                .inner
                .post(rpc_url)
                .header("Content-Type", "application/json")
                .json(&payload)
                .send()
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.as_u16() == 429 {
                        thread::sleep(Duration::from_secs(2u64.pow(attempt) * 2));
                        continue;
                    }
                    if !status.is_success() {
                        eprintln!("    RPC HTTP {status}");
                        return Ok(None);
                    }
                    return Ok(Some(resp.json()?));
                }
                Err(e) => {
                    eprintln!("    RPC error ({}/{}): {e}", attempt + 1, retries);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Ok(None)
    }

    pub fn get_current_block(&self, chain: &str) -> Result<u64> {
        let rpc = rpc_for_chain(chain);
        let r = self
            .rpc_call(rpc, "eth_blockNumber", json!([]), 3)?
            .context("eth_blockNumber failed")?;
        if let Some(result) = r.result {
            if let Some(hex) = result.as_str() {
                return Ok(parse_hex_u64(hex)?);
            }
        }
        Ok(0)
    }

    pub fn get_latest_block_and_ts(&self, rpc_url: &str) -> Result<(u64, i64)> {
        let r = self
            .rpc_call(rpc_url, "eth_getBlockByNumber", json!(["latest", false]), 4)?
            .context("eth_getBlockByNumber failed")?;
        let block = r
            .result
            .as_ref()
            .context("missing block result")?;
        let number = parse_hex_u64(block["number"].as_str().context("block number")?)?;
        let ts = parse_hex_u64(block["timestamp"].as_str().context("timestamp")?)? as i64;
        Ok((number, ts))
    }

    pub fn eth_call(&self, rpc_url: &str, contract: &str, data: &str) -> Result<Option<String>> {
        thread::sleep(Duration::from_millis(SLEEP_BETWEEN_RPC_MS));
        let r = self.rpc_call(
            rpc_url,
            "eth_call",
            json!([{"to": contract, "data": data}, "latest"]),
            3,
        )?;
        Ok(r.and_then(|r| r.result.and_then(|v| v.as_str().map(str::to_string))))
    }

    pub fn get_coingecko_price(&self, cg_id: &str) -> Result<Option<f64>> {
        thread::sleep(Duration::from_millis(SLEEP_BETWEEN_API_MS));
        let url = format!("{COINGECKO_BASE}/simple/price");
        let r = self.http_get(&url, &[("ids", cg_id), ("vs_currencies", "usd")], 3)?;
        Ok(r.and_then(|v| {
            v.get(cg_id)
                .and_then(|x| x.get("usd"))
                .and_then(|x| x.as_f64())
        }))
    }

    pub fn get_ethplorer_token_info(&self, contract: &str) -> Result<Value> {
        thread::sleep(Duration::from_millis(SLEEP_BETWEEN_API_MS));
        let url = format!("https://api.ethplorer.io/getTokenInfo/{contract}");
        Ok(self
            .http_get(&url, &[("apiKey", "freekey")], 3)?
            .filter(|v| v.get("error").is_none())
            .unwrap_or(json!({})))
    }

    pub fn get_ethplorer_top_holders(&self, contract: &str, limit: u32) -> Result<Vec<Value>> {
        thread::sleep(Duration::from_millis(SLEEP_BETWEEN_API_MS));
        let url = format!("https://api.ethplorer.io/getTopTokenHolders/{contract}");
        let limit_s = limit.to_string();
        let r = self.http_get(
            &url,
            &[("apiKey", "freekey"), ("limit", &limit_s)],
            3,
        )?;
        Ok(r.and_then(|v| {
            v.get("holders")
                .and_then(|h| serde_json::from_value(h.clone()).ok())
        })
        .unwrap_or_default())
    }

    pub fn get_transfer_logs_chunked(
        &self,
        contract: &str,
        chain: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        let rpc = rpc_for_chain(chain);
        let mut all_logs = Vec::new();
        let mut chunk = chunk_blocks;
        let mut current = from_block;
        let mut chunk_count = 0u64;
        let max_logs = 50_000usize;

        while current <= to_block && all_logs.len() < max_logs {
            let end = (current + chunk - 1).min(to_block);
            thread::sleep(Duration::from_millis(SLEEP_BETWEEN_RPC_MS));

            let params = json!([{
                "address": contract.to_lowercase(),
                "topics": [TRANSFER_TOPIC],
                "fromBlock": format!("0x{current:x}"),
                "toBlock": format!("0x{end:x}"),
            }]);

            let r = match self.rpc_call(rpc, "eth_getLogs", params, 3)? {
                Some(r) => r,
                None => {
                    eprintln!("    Chunk {chunk_count}: RPC call failed");
                    break;
                }
            };

            if let Some(err) = r.error {
                let msg = err.message.unwrap_or_default();
                if msg.contains("exceed maximum block range") {
                    chunk /= 2;
                    if chunk < 500 {
                        eprintln!("    Chunk too small, stopping: {msg}");
                        break;
                    }
                    eprintln!("    Block range exceeded, halving chunk to {chunk}");
                    continue;
                }
                eprintln!("    getLogs error: {msg}");
                break;
            }

            if let Some(batch) = r.result.and_then(|v| v.as_array().cloned()) {
                if !batch.is_empty() {
                    all_logs.extend(batch);
                }
            }

            chunk_count += 1;
            if chunk_count % 10 == 0 {
                eprintln!(
                    "    Chunk {chunk_count}: blocks 0x{current:x}-0x{end:x}, logs so far: {}",
                    all_logs.len()
                );
            }
            current = end + 1;
        }

        Ok(all_logs)
    }

    pub fn get_logs_activity(
        &self,
        rpc_url: &str,
        contract: &str,
        from_block: u64,
        to_block: u64,
        chunk_blocks: u64,
    ) -> Result<Vec<Value>> {
        let mut logs = Vec::new();
        let mut cur = from_block;
        let mut chunk = chunk_blocks;

        while cur <= to_block {
            let end = (cur + chunk - 1).min(to_block);
            let params = json!([{
                "address": contract,
                "topics": [TRANSFER_TOPIC],
                "fromBlock": format!("0x{cur:x}"),
                "toBlock": format!("0x{end:x}"),
            }]);

            match self.rpc_call(rpc_url, "eth_getLogs", params, 4) {
                Ok(Some(r)) => {
                    if let Some(err) = r.error {
                        let msg = err.message.unwrap_or_default();
                        if chunk > 5000 && (msg.to_lowercase().contains("range") || msg.to_lowercase().contains("limit")) {
                            chunk /= 2;
                            continue;
                        }
                        cur = end + 1;
                        continue;
                    }
                    if let Some(batch) = r.result.and_then(|v| v.as_array().cloned()) {
                        logs.extend(batch);
                    }
                    cur = end + 1;
                    thread::sleep(Duration::from_millis(100));
                }
                _ => {
                    if chunk > 5000 {
                        chunk /= 2;
                        continue;
                    }
                    cur = end + 1;
                }
            }
        }

        Ok(logs)
    }
}

pub fn decode_uint256(hex_str: Option<&str>) -> Option<u128> {
    let hex_str = hex_str?;
    if hex_str == "0x" {
        return None;
    }
    let s = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u128::from_str_radix(s, 16).ok()
}

pub fn parse_hex_u64(hex: &str) -> Result<u64> {
    let s = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(s, 16).map_err(|e| anyhow!("invalid hex {hex}: {e}"))
}

pub fn parse_transfer_log(log: &Value, anchor_ts: Option<i64>, anchor_block: Option<u64>, block_time: u64) -> Option<TransferLog> {
    let topics = log.get("topics")?.as_array()?;
    if topics.len() < 3 {
        return None;
    }

    let from_topic = topics[1].as_str()?;
    let to_topic = topics[2].as_str()?;
    let from_addr = format!("0x{}", &from_topic[from_topic.len().saturating_sub(40)..].to_lowercase());
    let to_addr = format!("0x{}", &to_topic[to_topic.len().saturating_sub(40)..].to_lowercase());

    let data = log.get("data").and_then(|d| d.as_str()).unwrap_or("0x");
    let value = if data == "0x" {
        0u128
    } else {
        u128::from_str_radix(data.strip_prefix("0x").unwrap_or(data), 16).unwrap_or(0)
    };

    let block_hex = log.get("blockNumber").and_then(|b| b.as_str());
    let block_number = block_hex.and_then(|h| parse_hex_u64(h).ok()).unwrap_or(0);

    let ts = if let Some(ts_hex) = log.get("timeStamp").and_then(|t| t.as_str()) {
        if ts_hex.starts_with("0x") {
            parse_hex_u64(ts_hex).ok()? as i64
        } else {
            ts_hex.parse().ok()?
        }
    } else if let (Some(anchor_ts), Some(anchor_block)) = (anchor_ts, anchor_block) {
        if block_number > 0 {
            anchor_ts - ((anchor_block.saturating_sub(block_number)) as i64 * block_time as i64)
        } else {
            return None;
        }
    } else {
        return None;
    };

    let dt = chrono::DateTime::from_timestamp(ts, 0)?;
    let year_month = dt.format("%Y-%m").to_string();

    Some(TransferLog {
        from: from_addr,
        to: to_addr,
        value,
        year_month,
        block_number,
    })
}

pub fn token_amount(raw: u128, decimals: u32) -> f64 {
    let divisor = 10f64.powi(decimals as i32);
    raw as f64 / divisor
}

pub fn default_fallback_block(chain: &str) -> u64 {
    if chain == "Ethereum" {
        25_200_000
    } else {
        57_000_000
    }
}

pub const ACTIVITY_CHUNK_BLOCKS: u64 = 20_000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ZERO_ADDRESS;
    use serde_json::json;

    fn transfer_log(from: &str, to: &str, data: &str, time_stamp: &str, block: &str) -> serde_json::Value {
        json!({
            "topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                format!("0x{:0>64}", from.strip_prefix("0x").unwrap_or(from)),
                format!("0x{:0>64}", to.strip_prefix("0x").unwrap_or(to)),
            ],
            "data": data,
            "timeStamp": time_stamp,
            "blockNumber": block,
        })
    }

    #[test]
    fn decode_uint256_parses_hex_words() {
        assert_eq!(decode_uint256(Some("0x0")), Some(0));
        assert_eq!(decode_uint256(Some("0x")), None);
        assert_eq!(decode_uint256(None), None);
        assert_eq!(decode_uint256(Some("0xf4240")), Some(1_000_000));
    }

    #[test]
    fn parse_hex_u64_accepts_prefixed_and_raw() {
        assert_eq!(parse_hex_u64("0x10").unwrap(), 16);
        assert_eq!(parse_hex_u64("10").unwrap(), 16);
        assert!(parse_hex_u64("0xzz").is_err());
    }

    #[test]
    fn token_amount_respects_decimals() {
        assert!((token_amount(1_000_000, 6) - 1.0).abs() < f64::EPSILON);
        assert!((token_amount(10u128.pow(18), 18) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn default_fallback_block_by_chain() {
        assert_eq!(default_fallback_block("Ethereum"), 25_200_000);
        assert_eq!(default_fallback_block("Polygon"), 57_000_000);
    }

    #[test]
    fn parse_transfer_log_from_timestamp_field() {
        let log = transfer_log(
            "0x0000000000000000000000000000000000000001",
            "0x0000000000000000000000000000000000000002",
            "0x64",
            "1704067200",
            "0x100",
        );
        let parsed = parse_transfer_log(&log, None, None, 12).unwrap();
        assert_eq!(parsed.from, "0x0000000000000000000000000000000000000001");
        assert_eq!(parsed.to, "0x0000000000000000000000000000000000000002");
        assert_eq!(parsed.value, 100);
        assert_eq!(parsed.year_month, "2024-01");
    }

    #[test]
    fn parse_transfer_log_estimates_timestamp_from_block_anchor() {
        let log = json!({
            "topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                format!("0x{:0>64}", ZERO_ADDRESS.strip_prefix("0x").unwrap()),
                "0x00000000000000000000000000000000000000aa",
            ],
            "data": "0x3e8",
            "blockNumber": "0x3e8",
        });
        let parsed = parse_transfer_log(&log, Some(1_000_000), Some(1000), 12).unwrap();
        assert_eq!(parsed.block_number, 1000);
        assert_eq!(parsed.year_month, "1970-01");
        assert_eq!(parsed.from, ZERO_ADDRESS);
    }

    #[test]
    fn parse_transfer_log_rejects_short_topics() {
        let log = json!({"topics": ["0xabc"], "data": "0x1"});
        assert!(parse_transfer_log(&log, None, None, 12).is_none());
    }
}
