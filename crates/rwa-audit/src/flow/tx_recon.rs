use anyhow::{Context, Result};

use crate::client::{parse_hex_u64, HttpClient};
use crate::config::{ensure_dir, ZERO_ADDRESS};
use crate::flow::config::flow_data_dir;
use crate::flow::output::{write_tx_reconstructions, TxReconRow};

const SWAP_V3_TOPIC: &str = "0xc42079f94a6350d65e623abf017871ec234316d7fcc48fd4af35ff82926fdc145";

pub(crate) fn reconstruct_case_studies(extra_hashes: &[String]) -> Result<()> {
    let out_dir = flow_data_dir();
    ensure_dir(&out_dir)?;

    let client = HttpClient::new()?;
    let mut rows = Vec::new();

    for hash in extra_hashes {
        println!("Reconstructing {hash}...");
        if let Some(row) = reconstruct_tx(&client, hash, "cli")? {
            rows.push(row);
        }
    }

    if rows.is_empty() {
        eprintln!("No transaction hashes supplied. Pass hashes as CLI arguments.");
        eprintln!("Example: cargo run --bin rwa-flow-tx -- 0x<full_hash>");
    }

    write_tx_reconstructions(&out_dir, &rows)?;
    println!(
        "Wrote {}",
        out_dir.join("tx_reconstructions.json").display()
    );
    Ok(())
}

fn reconstruct_tx(client: &HttpClient, hash: &str, label: &str) -> Result<Option<TxReconRow>> {
    let eth_rpc = client.context().rpc_for_chain("Ethereum")?;
    let r = client
        .rpc_call(
            &eth_rpc,
            "eth_getTransactionReceipt",
            serde_json::json!([hash]),
            3,
        )?
        .context("getTransactionReceipt")?;

    let receipt = match r.result {
        Some(v) if !v.is_null() => v,
        _ => {
            eprintln!("  Receipt not found for {hash}");
            return Ok(None);
        }
    };

    let block = receipt
        .get("blockNumber")
        .and_then(|b| b.as_str())
        .and_then(|h| parse_hex_u64(h).ok())
        .unwrap_or(0);

    let logs = receipt
        .get("logs")
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default();

    let mut transfer_count = 0u32;
    let mut mint_count = 0u32;
    let mut burn_count = 0u32;
    let mut swap_count = 0u32;
    let mut unique_recipients = std::collections::HashSet::new();
    let mut token_contracts = std::collections::HashSet::new();
    let mut log_summaries = Vec::new();

    let zero = ZERO_ADDRESS.to_lowercase();

    for log in &logs {
        let topics: Vec<String> = log
            .get("topics")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        if topics.is_empty() {
            continue;
        }

        let contract = log
            .get("address")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_lowercase();
        token_contracts.insert(contract.clone());

        if topics[0].eq_ignore_ascii_case(crate::config::TRANSFER_TOPIC) {
            transfer_count += 1;
            if topics.len() >= 3 {
                let from = topic_address(&topics[1]);
                let to = topic_address(&topics[2]);
                if from == zero {
                    mint_count += 1;
                }
                if to == zero {
                    burn_count += 1;
                }
                unique_recipients.insert(to.clone());
                let value = log
                    .get("data")
                    .and_then(|d| d.as_str())
                    .and_then(|h| u128::from_str_radix(h.strip_prefix("0x").unwrap_or(h), 16).ok())
                    .unwrap_or(0);
                if log_summaries.len() < 20 {
                    log_summaries.push(format!("Transfer {contract}: {from} → {to} value={value}"));
                }
            }
        } else if topics[0].eq_ignore_ascii_case(SWAP_V3_TOPIC) {
            swap_count += 1;
            if log_summaries.len() < 20 {
                log_summaries.push(format!("UniswapV3 Swap pool={contract}"));
            }
        }
    }

    Ok(Some(TxReconRow {
        label: label.to_string(),
        tx_hash: hash.to_string(),
        block_number: block,
        log_count: logs.len() as u32,
        transfer_count,
        mint_count,
        burn_count,
        swap_count,
        unique_transfer_recipients: unique_recipients.len() as u32,
        distinct_log_contracts: token_contracts.len() as u32,
        log_summaries,
        source: "publicnode eth_getTransactionReceipt".into(),
    }))
}

fn topic_address(topic: &str) -> String {
    format!(
        "0x{}",
        &topic[topic.len().saturating_sub(40)..].to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_address_extracts_last_20_bytes() {
        let t = "0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        assert!(topic_address(t).ends_with("deadbeef"));
    }

    #[test]
    fn topic_address_short_string_does_not_panic() {
        let result = topic_address("0xabcd");
        assert!(result.starts_with("0x"));
    }

    #[test]
    fn topic_address_empty_does_not_panic() {
        let result = topic_address("");
        assert_eq!(result, "0x");
    }

    #[test]
    fn topic_address_lowercase() {
        let t = "0x000000000000000000000000ABCDEF1234567890ABCDEF1234567890ABCDEF12";
        let addr = topic_address(t);
        assert_eq!(addr, addr.to_lowercase());
    }
}
