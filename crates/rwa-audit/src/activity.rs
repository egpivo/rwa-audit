use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{Duration, Utc};
use csv::ReaderBuilder;

use crate::assets::activity_assets;
use crate::client::{parse_hex_u64, token_amount, HttpClient, ACTIVITY_CHUNK_BLOCKS};
use crate::config::{block_time_secs, data_dir, ensure_dir, rpc_for_chain};
use crate::models::ActivityDailyRow;
use crate::output::write_activity_daily;

struct DailyBucket {
    volume_tokens: f64,
    senders: HashSet<String>,
}

pub fn collect_activity_timeseries() -> anyhow::Result<()> {
    let output_path = data_dir().join("rwa_activity_daily_30d.csv");
    ensure_dir(output_path.parent().unwrap())?;

    let implied_price = load_implied_prices(&data_dir().join("rwa_transfer_metrics.csv"))?;
    let client = HttpClient::new()?;
    let mut rows = Vec::new();

    for asset in activity_assets() {
        let rpc = rpc_for_chain(&asset.chain);
        let (latest_block, latest_ts) = client.get_latest_block_and_ts(rpc)?;
        let sec_per_block = block_time_secs(&asset.chain);
        let back_blocks = (30 * 86400) / sec_per_block;
        let from_block = latest_block.saturating_sub(back_blocks);

        println!(
            "Collecting {} logs from {from_block} to {latest_block}...",
            asset.symbol
        );
        let logs = client.get_logs_activity(
            rpc,
            &asset.contract,
            from_block,
            latest_block,
            ACTIVITY_CHUNK_BLOCKS,
        )?;
        println!("Collected {} logs for {}", logs.len(), asset.symbol);

        let mut daily: HashMap<String, DailyBucket> = HashMap::new();
        for lg in &logs {
            let bn = lg
                .get("blockNumber")
                .and_then(|b| b.as_str())
                .and_then(|h| parse_hex_u64(h).ok())
                .unwrap_or(0);
            let ts = latest_ts - ((latest_block.saturating_sub(bn)) as i64 * sec_per_block as i64);
            let day = chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.date_naive().to_string())
                .unwrap_or_default();

            let topics = lg.get("topics").and_then(|t| t.as_array());
            if topics.map(|t| t.len()).unwrap_or(0) < 3 {
                continue;
            }
            let from_topic = topics.unwrap()[1].as_str().unwrap_or("");
            let from_addr = format!(
                "0x{}",
                &from_topic[from_topic.len().saturating_sub(40)..].to_lowercase()
            );

            let data = lg.get("data").and_then(|d| d.as_str()).unwrap_or("0x0");
            let raw = u128::from_str_radix(data.strip_prefix("0x").unwrap_or(data), 16).unwrap_or(0);
            let bucket = daily.entry(day).or_insert_with(|| DailyBucket {
                volume_tokens: 0.0,
                senders: HashSet::new(),
            });
            bucket.volume_tokens += token_amount(raw, asset.decimals);
            bucket.senders.insert(from_addr);
        }

        let end_date = chrono::DateTime::from_timestamp(latest_ts, 0)
            .map(|dt| dt.date_naive())
            .unwrap_or_else(|| Utc::now().date_naive());
        let start_date = end_date - Duration::days(29);

        for i in 0..30 {
            let d = start_date + Duration::days(i);
            let key = d.to_string();
            let token_vol = daily.get(&key).map(|b| b.volume_tokens).unwrap_or(0.0);
            let px = asset
                .price_usd_approx
                .or_else(|| implied_price.get(&asset.symbol).copied());
            let usd_vol = px.map(|p| token_vol * p);
            let sender_count = daily.get(&key).map(|b| b.senders.len()).unwrap_or(0) as u64;

            let workflow_type = if asset.symbol == "BENJI" {
                "Transfer-agent recordkeeping extension"
            } else {
                "Token-level ERC-20 observable activity"
            };

            let volume_metric_type = if asset.price_usd_approx.is_some() {
                "Daily ERC-20 transfer volume (USD approx)"
            } else {
                "Daily ERC-20 transfer volume (token units; USD unavailable in this extraction)"
            };

            rows.push(ActivityDailyRow {
                date: key,
                product_or_platform: asset.symbol.clone(),
                workflow_type: workflow_type.into(),
                chain_or_venue: asset.chain.clone(),
                volume_metric_type: volume_metric_type.into(),
                volume_usd: usd_vol
                    .map(|v| format!("{v:.6}"))
                    .unwrap_or_default(),
                volume_tokens: format!("{token_vol:.6}"),
                active_user_metric_type: "Daily unique senders".into(),
                active_user_count: sender_count,
                observation_domain: format!(
                    "Contract={}; Chain={}; Event=Transfer; Window=daily UTC",
                    asset.contract, asset.chain
                ),
                source: "publicnode RPC eth_getLogs; timestamp estimated from block distance to latest block".into(),
                include_in_figure: if asset.include_in_figure {
                    "yes"
                } else {
                    "no"
                }
                .into(),
            });
        }
    }

    write_activity_daily(&output_path, &rows)?;
    println!("Wrote {}", output_path.display());
    Ok(())
}

pub(crate) fn load_implied_prices(path: &Path) -> anyhow::Result<HashMap<String, f64>> {
    let mut out = HashMap::new();
    if !path.exists() {
        return Ok(out);
    }

    let mut rdr = ReaderBuilder::new().from_path(path)?;
    for result in rdr.records() {
        let r = result?;
        if r.get(2) != Some("2026-05") {
            continue;
        }
        let usd = r.get(7).filter(|s| !s.is_empty() && *s != "N/A");
        let tok = r.get(6);
        if let (Some(usd_s), Some(tok_s), Some(sym)) = (usd, tok, r.get(1)) {
            if let (Ok(u), Ok(t)) = (usd_s.parse::<f64>(), tok_s.parse::<f64>()) {
                if t > 0.0 {
                    out.insert(sym.to_string(), u / t);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn load_implied_prices_missing_file_returns_empty() {
        let prices = load_implied_prices(Path::new("/nonexistent/rwa_transfer_metrics.csv")).unwrap();
        assert!(prices.is_empty());
    }

    #[test]
    fn load_implied_prices_parses_matching_month() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-audit-activity-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("transfer.csv");
        std::fs::write(
            &path,
            "asset_name,symbol,year_month,transfer_count,unique_senders,unique_receivers,total_volume_tokens,total_volume_usd_approx\n\
             Ondo USDY,USDY,2026-05,10,5,5,1000.0,1050.0\n\
             Paxos Gold,PAXG,2026-04,1,1,1,10.0,20000.0\n",
        )
        .unwrap();

        let prices = load_implied_prices(&path).unwrap();
        assert_eq!(prices.len(), 1);
        assert!((prices["USDY"] - 1.05).abs() < f64::EPSILON);

        let _ = std::fs::remove_dir_all(dir);
    }
}
