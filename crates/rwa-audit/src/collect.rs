use std::path::Path;

use chrono::Utc;

use crate::assets::{detect_permissioning_from_known, registry_assets_from};
use crate::client::{decode_uint256, default_fallback_block, HttpClient};
use crate::config::{
    block_time_secs, data_dir, ensure_dir, rpc_for_chain, CHUNK_BLOCKS, MONTHS_HISTORY,
    TOTAL_SUPPLY_SEL,
};
use crate::metrics::compute_monthly_metrics;
use crate::models::{HolderRow, MintBurnRow, QualityNote, RegistryRow, TransferRow};
use crate::output::{
    write_holder_metrics, write_mint_burn_metrics, write_quality_notes, write_registry,
    write_transfer_metrics,
};

pub(crate) fn total_supply_tokens_from_raw(raw: Option<u128>, decimals: u32) -> Option<f64> {
    raw.map(|r| r as f64 / 10f64.powi(decimals as i32))
}

pub(crate) fn resolve_price_usd(
    coin_gecko_price: Option<f64>,
    hardcoded: Option<f64>,
) -> (Option<f64>, Option<String>) {
    if let Some(p) = coin_gecko_price {
        return (Some(p), None);
    }
    if let Some(h) = hardcoded {
        return (
            Some(h),
            Some(format!("Price USD used: {h} (hardcoded approximate).")),
        );
    }
    (None, None)
}

pub(crate) fn history_from_block(current_block: u64, block_time_sec: u64, months: u64) -> u64 {
    let blocks_per_second = 1.0 / block_time_sec as f64;
    let blocks_per_month = (86400.0 * 30.0 * blocks_per_second) as u64;
    current_block.saturating_sub(blocks_per_month * months)
}

pub(crate) fn holder_concentration(top_holders: &[serde_json::Value]) -> (String, String) {
    let shares: Vec<f64> = top_holders
        .iter()
        .filter_map(|h| h.get("share").and_then(|s| s.as_f64()))
        .collect();
    if shares.is_empty() {
        return ("N/A".into(), "N/A".into());
    }
    (
        format!("{:.2}", shares.iter().sum::<f64>()),
        format!("{:.2}", shares[0]),
    )
}

pub(crate) fn round_metric(value: f64) -> f64 {
    (value * 10000.0).round() / 10000.0
}

pub fn collect_all(assets_path: &Path) -> anyhow::Result<()> {
    let output_dir = data_dir();
    ensure_dir(&output_dir)?;

    let client = HttpClient::for_live()?;
    let assets = registry_assets_from(assets_path)?;
    let mut registry_rows = Vec::new();
    let mut transfer_rows = Vec::new();
    let mut holder_rows = Vec::new();
    let mut mint_burn_rows = Vec::new();
    let mut quality_notes = Vec::new();

    for asset in assets {
        let name = asset.asset_name.clone();
        let symbol = asset.symbol.clone();
        let chain = asset.chain.clone();
        let contract = asset.contract_address.to_lowercase();
        let cg_id = asset.coingecko_id.clone();
        let category = asset.category.clone();
        let decimals = asset.decimals;
        let notes_base = asset.notes.clone();
        let price_hardcoded = asset.price_usd_approx;

        println!("\n{}", "=".repeat(60));
        println!("Processing: {name} ({symbol}) on {chain}");
        println!("  Contract: {contract}");

        let mut data_issues = Vec::new();
        let mut context_notes = if notes_base.is_empty() {
            Vec::new()
        } else {
            vec![notes_base.clone()]
        };

        let rpc_url = rpc_for_chain(&chain);
        let block_time_sec = block_time_secs(&chain);

        let mut current_block = client.get_current_block(&chain)?;
        println!("  Current block: {current_block}");
        if current_block == 0 {
            data_issues.push("Could not fetch current block number from RPC.".into());
            current_block = default_fallback_block(&chain);
        }

        println!("  Fetching total supply via eth_call...");
        let ts_raw = client.eth_call(rpc_url, &contract, TOTAL_SUPPLY_SEL)?;
        let total_supply_raw = decode_uint256(ts_raw.as_deref());

        let mut total_supply_tokens = total_supply_tokens_from_raw(total_supply_raw, decimals);

        if total_supply_tokens.is_none() {
            data_issues.push("total_supply eth_call returned null or zero.".into());
        }

        println!("  Fetching price (CoinGecko id: {:?})...", cg_id);
        let mut price_usd = if let Some(ref id) = cg_id {
            match client.get_coingecko_price(id)? {
                Some(p) => Some(p),
                None => {
                    data_issues.push(format!("CoinGecko price fetch failed for {id}."));
                    None
                }
            }
        } else {
            None
        };

        let (resolved_price, price_note) = resolve_price_usd(price_usd, price_hardcoded);
        price_usd = resolved_price;
        if let Some(note) = price_note {
            context_notes.push(note);
        }

        let total_supply_usd = match (total_supply_tokens, price_usd) {
            (Some(ts), Some(px)) => format!("{:.2}", ts * px),
            _ => "N/A".into(),
        };

        let is_permissioned = detect_permissioning_from_known(&symbol);
        context_notes.push(format!(
            "Permissioned (public docs): {:?}.",
            is_permissioned
        ));

        println!("  Fetching Ethplorer token info and top holders...");
        let (ethplorer_info, top_holders) = if chain == "Ethereum" {
            (
                client.get_ethplorer_token_info(&contract)?,
                client.get_ethplorer_top_holders(&contract, 10)?,
            )
        } else {
            data_issues.push(format!(
                "Ethplorer does not support {chain}; holder data unavailable."
            ));
            (serde_json::json!({}), Vec::new())
        };

        let mut holder_count = "N/A".to_string();
        if let Some(hc) = ethplorer_info.get("holdersCount") {
            if let Some(n) = hc
                .as_u64()
                .or_else(|| hc.as_str().and_then(|s| s.parse().ok()))
            {
                holder_count = n.to_string();
            } else {
                data_issues.push("holdersCount parse error from Ethplorer.".into());
            }
        }

        if total_supply_tokens.is_none() {
            if let Some(ts_ep) = ethplorer_info.get("totalSupply") {
                if let Some(s) = ts_ep.as_str().and_then(|s| s.parse::<f64>().ok()) {
                    total_supply_tokens = Some(s / 10f64.powi(decimals as i32));
                }
            }
        }

        let mut top10_pct = "N/A".to_string();
        let mut top1_pct = "N/A".to_string();
        if !top_holders.is_empty() && total_supply_tokens.is_some() {
            (top10_pct, top1_pct) = holder_concentration(&top_holders);
        } else if top_holders.is_empty() && chain == "Ethereum" {
            data_issues.push("Top holder data not available from Ethplorer.".into());
        }

        println!("  Fetching transfer logs ({MONTHS_HISTORY} months history)...");
        let from_block = history_from_block(current_block, block_time_sec, MONTHS_HISTORY);
        let history_blocks = current_block.saturating_sub(from_block);
        println!("  Block range: {from_block} to {current_block} ({history_blocks} blocks)");

        let logs = client.get_transfer_logs_chunked(
            &contract,
            &chain,
            from_block,
            current_block,
            CHUNK_BLOCKS,
        )?;
        println!("  Total logs fetched: {}", logs.len());

        if logs.is_empty() {
            data_issues.push(format!(
                "No transfer logs found in last {MONTHS_HISTORY} months."
            ));
        }

        let monthly = compute_monthly_metrics(&logs, decimals);
        for ym in monthly.keys().cloned().collect::<Vec<_>>().into_iter() {
            let m = &monthly[&ym];
            let vol_tokens = m.total_volume_tokens;
            let vol_usd = price_usd
                .map(|px| format!("{:.2}", vol_tokens * px))
                .unwrap_or_else(|| "N/A".into());

            transfer_rows.push(TransferRow {
                asset_name: name.clone(),
                symbol: symbol.clone(),
                year_month: ym.clone(),
                transfer_count: m.transfer_count,
                unique_senders: m.unique_senders.len(),
                unique_receivers: m.unique_receivers.len(),
                total_volume_tokens: round_metric(vol_tokens),
                total_volume_usd_approx: vol_usd,
            });

            mint_burn_rows.push(MintBurnRow {
                asset_name: name.clone(),
                symbol: symbol.clone(),
                year_month: ym,
                mint_count: m.mint_count,
                mint_volume_tokens: round_metric(m.mint_volume),
                burn_count: m.burn_count,
                burn_volume_tokens: round_metric(m.burn_volume),
                net_issuance_tokens: round_metric(m.mint_volume - m.burn_volume),
            });
        }

        registry_rows.push(RegistryRow {
            asset_name: name.clone(),
            symbol: symbol.clone(),
            category,
            chain: chain.clone(),
            contract_address: contract,
            decimals,
            total_supply: total_supply_tokens
                .map(|t| format!("{:.4}", t))
                .unwrap_or_else(|| "N/A".into()),
            total_supply_usd_approx: total_supply_usd,
            is_permissioned: is_permissioned.unwrap_or_else(|| "N/A".into()),
            data_source: "publicnode RPC + Ethplorer API".into(),
            notes: context_notes
                .iter()
                .filter(|n| !n.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" | "),
        });

        holder_rows.push(HolderRow {
            asset_name: name,
            symbol: symbol.clone(),
            holder_count,
            top10_concentration_pct: top10_pct,
            top1_concentration_pct: top1_pct,
            data_as_of: Utc::now().date_naive().to_string(),
            data_source: if chain == "Ethereum" {
                "Ethplorer freekey API".into()
            } else {
                format!("{chain} - data not available")
            },
        });

        let issue_count = data_issues.len();
        if !data_issues.is_empty() {
            quality_notes.push(QualityNote {
                name: asset.asset_name,
                symbol,
                chain,
                issues: data_issues,
                context: context_notes,
            });
        }

        println!("  Done. Issues: {issue_count}");
    }

    println!("\n\nWriting output files...");
    write_registry(&output_dir, &registry_rows)?;
    write_transfer_metrics(&output_dir, &transfer_rows)?;
    write_holder_metrics(&output_dir, &holder_rows)?;
    write_mint_burn_metrics(&output_dir, &mint_burn_rows)?;
    write_quality_notes(&output_dir, &quality_notes)?;

    println!("\nData collection complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn total_supply_tokens_from_raw_scales_decimals() {
        assert!(
            (total_supply_tokens_from_raw(Some(1_000_000), 6).unwrap() - 1.0).abs() < f64::EPSILON
        );
        assert!(total_supply_tokens_from_raw(None, 18).is_none());
    }

    #[test]
    fn resolve_price_usd_prefers_coingecko() {
        let (price, note) = resolve_price_usd(Some(1.05), Some(1.0));
        assert!((price.unwrap() - 1.05).abs() < f64::EPSILON);
        assert!(note.is_none());
    }

    #[test]
    fn resolve_price_usd_falls_back_to_hardcoded() {
        let (price, note) = resolve_price_usd(None, Some(1.0));
        assert!((price.unwrap() - 1.0).abs() < f64::EPSILON);
        assert!(note.unwrap().contains("hardcoded"));
    }

    #[test]
    fn history_from_block_covers_six_months_on_ethereum() {
        let from = history_from_block(10_000_000, 12, 6);
        assert_eq!(from, 10_000_000 - 1_296_000);
    }

    #[test]
    fn holder_concentration_sums_top_shares() {
        let holders = vec![
            json!({"share": 25.5}),
            json!({"share": 10.0}),
            json!({"share": 4.5}),
        ];
        let (top10, top1) = holder_concentration(&holders);
        assert_eq!(top10, "40.00");
        assert_eq!(top1, "25.50");
    }

    #[test]
    fn round_metric_four_decimal_places() {
        assert!((round_metric(1.234567) - 1.2346).abs() < f64::EPSILON);
    }
}
