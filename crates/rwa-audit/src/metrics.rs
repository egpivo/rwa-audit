use std::collections::HashMap;

use serde_json::Value;

use crate::client::{parse_transfer_log, token_amount};
use crate::config::ZERO_ADDRESS;
use crate::models::{MonthlyMetrics, TransferLog};

pub fn compute_monthly_metrics(logs: &[Value], decimals: u32) -> HashMap<String, MonthlyMetrics> {
    let zero = ZERO_ADDRESS.to_lowercase();
    let mut monthly: HashMap<String, MonthlyMetrics> = HashMap::new();

    for raw in logs {
        let Some(parsed) = parse_transfer_log(raw, None, None, 12) else {
            continue;
        };
        accumulate(&mut monthly, &parsed, decimals, &zero);
    }

    monthly
}

fn accumulate(
    monthly: &mut HashMap<String, MonthlyMetrics>,
    parsed: &TransferLog,
    decimals: u32,
    zero: &str,
) {
    let m = monthly.entry(parsed.year_month.clone()).or_default();
    m.transfer_count += 1;
    m.unique_senders.insert(parsed.from.clone());
    m.unique_receivers.insert(parsed.to.clone());

    let token_value = token_amount(parsed.value, decimals);
    m.total_volume_tokens += token_value;

    if parsed.from == zero {
        m.mint_count += 1;
        m.mint_volume += token_value;
    }
    if parsed.to == zero {
        m.burn_count += 1;
        m.burn_volume += token_value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ZERO_ADDRESS;
    use serde_json::json;

    fn mint_log(value_hex: &str, month_ts: &str) -> Value {
        json!({
            "topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                format!("0x{:0>64}", ZERO_ADDRESS.strip_prefix("0x").unwrap()),
                "0x00000000000000000000000000000000000000bb",
            ],
            "data": value_hex,
            "timeStamp": month_ts,
            "blockNumber": "0x1",
        })
    }

    #[test]
    fn compute_monthly_metrics_counts_mint_burn_and_uniques() {
        let logs = vec![
            mint_log("0x64", "1704067200"), // 100 tokens @ 18 dec in Jan 2024 if decimals=0 for test
            json!({
                "topics": [
                    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                    "0x00000000000000000000000000000000000000aa",
                    format!("0x{:0>64}", ZERO_ADDRESS.strip_prefix("0x").unwrap()),
                ],
                "data": "0x32",
                "timeStamp": "1704067200",
                "blockNumber": "0x2",
            }),
        ];

        let monthly = compute_monthly_metrics(&logs, 0);
        let m = monthly.get("2024-01").expect("month bucket");
        assert_eq!(m.transfer_count, 2);
        assert_eq!(m.mint_count, 1);
        assert_eq!(m.burn_count, 1);
        assert_eq!(m.unique_senders.len(), 2);
        assert_eq!(m.unique_receivers.len(), 2);
        assert!((m.mint_volume - 100.0).abs() < f64::EPSILON);
        assert!((m.burn_volume - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn compute_monthly_metrics_skips_unparseable_logs() {
        let logs = vec![json!({"topics": []})];
        assert!(compute_monthly_metrics(&logs, 18).is_empty());
    }
}
