use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use serde_json::Value;

use crate::config::ensure_dir;
use crate::flow::config::{flow_data_dir, PanelToken, PANEL_TOKENS, PARASWAP_BASE, PARASWAP_SLEEP_MS, QUOTE_USD_SIZES, USDC};
use crate::flow::output::{write_paraswap_quotes, ParaswapQuoteRow};

pub fn collect_paraswap_quotes() -> Result<()> {
    let out_dir = flow_data_dir();
    ensure_dir(&out_dir)?;

    let http = Client::builder()
        .user_agent("rwa-audit/0.1")
        .timeout(Duration::from_secs(30))
        .build()?;

    let checkpoint = chrono::Utc::now().date_naive().to_string();
    let mut rows = Vec::new();

    for token in PANEL_TOKENS {
        for &usd_size in QUOTE_USD_SIZES {
            let token_amount = usd_size as f64 / token.approx_price_usd;
            let raw_amount = (token_amount * 10f64.powi(token.decimals as i32)) as u128;

            thread::sleep(Duration::from_millis(PARASWAP_SLEEP_MS));
            let (route_found, dest_amount, route_summary, error_msg) =
                fetch_quote(&http, token, raw_amount)?;

            rows.push(ParaswapQuoteRow {
                checkpoint_date: checkpoint.clone(),
                symbol: token.symbol.to_string(),
                src_token: token.address.to_string(),
                dest_token: USDC.to_string(),
                amount_usd: usd_size,
                route_found,
                dest_amount_usdc: dest_amount,
                route_summary,
                error_message: error_msg,
                source: "ParaSwap API v5 live quote".into(),
            });

            println!(
                "  {} ${usd_size}: route_found={route_found}",
                token.symbol
            );
        }
    }

    write_paraswap_quotes(&out_dir, &rows)?;
    println!("Wrote {}", out_dir.join("paraswap_quotes.csv").display());
    Ok(())
}

fn fetch_quote(
    http: &Client,
    token: &PanelToken,
    raw_amount: u128,
) -> Result<(bool, Option<f64>, String, Option<String>)> {
    let url = format!("{PARASWAP_BASE}/prices/");
    let resp = http
        .get(&url)
        .query(&[
            ("srcToken", token.address),
            ("destToken", USDC),
            ("amount", &raw_amount.to_string()),
            ("srcDecimals", &token.decimals.to_string()),
            ("destDecimals", "6"),
            ("side", "SELL"),
            ("network", "1"),
        ])
        .send()
        .context("ParaSwap prices")?;

    let status = resp.status();
    let body: Value = resp.json().unwrap_or(Value::Null);

    if !status.is_success() {
        let err = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("no route")
            .to_string();
        return Ok((false, None, String::new(), Some(err)));
    }

    let route = body.get("priceRoute").context("priceRoute")?;
    let dest_raw = route
        .get("destAmount")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .map(|a| a / 1e6);

    let exchanges: Vec<String> = route
        .pointer("/bestRoute/0/swaps")
        .and_then(|s| s.as_array())
        .map(|swaps| {
            swaps
                .iter()
                .filter_map(|sw| {
                    sw.get("swapExchanges")
                        .and_then(|e| e.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|ex| ex.get("exchange"))
                        .and_then(|e| e.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();

    Ok((
        true,
        dest_raw,
        exchanges.join(" → "),
        None,
    ))
}
