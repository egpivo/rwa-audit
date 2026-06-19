use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::models::TransferLog;

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

pub fn parse_transfer_log(
    log: &Value,
    anchor_ts: Option<i64>,
    anchor_block: Option<u64>,
    block_time: u64,
) -> Option<TransferLog> {
    let topics = log.get("topics")?.as_array()?;
    if topics.len() < 3 {
        return None;
    }

    let from_topic = topics[1].as_str()?;
    let to_topic = topics[2].as_str()?;
    let from_addr = format!(
        "0x{}",
        &from_topic[from_topic.len().saturating_sub(40)..].to_lowercase()
    );
    let to_addr = format!(
        "0x{}",
        &to_topic[to_topic.len().saturating_sub(40)..].to_lowercase()
    );

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

    fn transfer_log(
        from: &str,
        to: &str,
        data: &str,
        time_stamp: &str,
        block: &str,
    ) -> serde_json::Value {
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
