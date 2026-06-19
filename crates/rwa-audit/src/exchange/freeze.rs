use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::blocking::Client;

use crate::config::ensure_dir;
use crate::core::manifest::{AuditManifest, ManifestClaim};
use crate::exchange::bridged::sum_bridged_value_for_date;
use crate::exchange::config::{
    self, BRIDGED_VALUE_DATE, PLATFORM_TRANSFER_APR_DATE, PLATFORM_TRANSFER_JUN_DATE,
    PUBLISH_PANEL_DATE, XSTOCKS_SOLANA,
};
use crate::exchange::output::{
    bridged_row, gecko_row, jupiter_row, platform_row, write_depth_panel, write_json,
    write_sourced_json, DepthPanelRow, ExchangeManifest,
};
use crate::exchange::reference::{
    gecko_aggregate_from_reference, jupiter_quote_from_publish_fixture,
};
use crate::exchange::rwa_xyz::{
    fetch_and_merge_seed, load_seed, save_seed_to, snapshot_for_date, SEED_TRANSFER_FILENAME,
};
use crate::flow::config::JUPITER_QUOTE_USD;
use crate::flow::gecko::{fetch_solana_symbol_pool_aggregate, SymbolPoolAggregate};
use crate::flow::jupiter::{fetch_aaplx_quote_100k, JupiterQuoteEvidence};
use crate::sources::SourceId;

#[derive(Debug, Clone, Default)]
pub struct ExchangeFreezeOptions {
    pub refresh_rwa_xyz: bool,
    /// Fetch Gecko + Jupiter live APIs. Default uses regression panel + publish fixture.
    pub live_apis: bool,
    /// Panel / claim `as_of` label; defaults to publish panel date when unset.
    pub panel_date: Option<String>,
}

/// When live staging runs without `--refresh-rwa`, copy the loaded publish seed beside
/// other evidence so manifest `evidence_file` paths resolve.
pub(crate) fn materialize_transfer_seed_for_live_staging(
    live_apis: bool,
    refresh_rwa_xyz: bool,
    out_dir: &std::path::Path,
    seed: &crate::exchange::rwa_xyz::PlatformSeedFile,
) -> Result<()> {
    if live_apis && !refresh_rwa_xyz {
        save_seed_to(&out_dir.join(SEED_TRANSFER_FILENAME), seed)?;
    }
    Ok(())
}

pub fn freeze_exchange_evidence(opts: ExchangeFreezeOptions) -> Result<()> {
    let out_dir = config::exchange_output_dir(opts.live_apis);
    ensure_dir(&out_dir)?;

    let panel_date = config::resolve_panel_date(opts.live_apis, opts.panel_date.as_deref())?;

    let accessed = panel_date.clone();
    let frozen_at = frozen_at_timestamp();

    let seed = if opts.refresh_rwa_xyz {
        println!("Fetching RWA.xyz platform snapshots...");
        let save_path = if opts.live_apis {
            out_dir.join(SEED_TRANSFER_FILENAME)
        } else {
            config::platform_seed_path()
        };
        fetch_and_merge_seed(&accessed, &save_path)?
    } else {
        load_seed().context("load rwa_xyz seed — run with --refresh-rwa or copy seeds/")?
    };

    materialize_transfer_seed_for_live_staging(
        opts.live_apis,
        opts.refresh_rwa_xyz,
        &out_dir,
        &seed,
    )?;

    let apr = snapshot_for_date(&seed, PLATFORM_TRANSFER_APR_DATE)
        .context("missing April 2026 platform transfer snapshot")?;
    let jun = snapshot_for_date(&seed, PLATFORM_TRANSFER_JUN_DATE)
        .context("missing June 2026 platform transfer snapshot")?;

    write_json(&out_dir.join("rwa_xyz_platform_snapshots.json"), &seed)?;

    println!("Summing bridged token value ({BRIDGED_VALUE_DATE})...");
    let bridged = sum_bridged_value_for_date(BRIDGED_VALUE_DATE)?;
    write_json(&out_dir.join("bridged_value_sum.json"), &bridged)?;

    let gecko_aggs = if opts.live_apis {
        let http = Client::builder()
            .user_agent("rwa-audit/0.1")
            .timeout(std::time::Duration::from_secs(60))
            .build()?;
        let mut aggs = Vec::new();
        for x in XSTOCKS_SOLANA {
            println!("GeckoTerminal Solana pools for {}...", x.symbol);
            let agg = fetch_solana_symbol_pool_aggregate(&http, x.symbol)?;
            let fname = format!("gecko_{}_pools.json", x.symbol.to_lowercase());
            write_sourced_json(
                &out_dir.join(&fname),
                &agg,
                SourceId::GeckoTerminal,
                &agg.source_url,
                true,
            )?;
            aggs.push(agg);
        }
        aggs
    } else {
        println!("Loading Gecko xStocks aggregates from reference panel ({PUBLISH_PANEL_DATE})...");
        let mut aggs = Vec::new();
        for x in XSTOCKS_SOLANA {
            let agg = gecko_aggregate_from_reference(x.symbol)?;
            let fname = format!("gecko_{}_pools.json", x.symbol.to_lowercase());
            write_json(&out_dir.join(&fname), &agg)?;
            aggs.push(agg);
        }
        aggs
    };

    let jupiter = if opts.live_apis {
        println!("Jupiter USDC → AAPLx @ ${JUPITER_QUOTE_USD}...");
        let q = fetch_aaplx_quote_100k()?;
        write_sourced_json(
            &out_dir.join("jupiter_quote_aaplx_100k.json"),
            &q,
            SourceId::Jupiter,
            &q.source_url,
            true,
        )?;
        q
    } else {
        println!("Loading Jupiter $100k AAPLx quote from publish fixture...");
        let q = jupiter_quote_from_publish_fixture()?;
        write_json(&out_dir.join("jupiter_quote_aaplx_100k.json"), &q)?;
        q
    };

    let mut panel_rows: Vec<DepthPanelRow> = vec![
        platform_row(apr, &accessed),
        platform_row(jun, &accessed),
        bridged_row(&bridged, &accessed),
    ];
    for agg in &gecko_aggs {
        panel_rows.push(gecko_row(agg, "pool_tvl_total", &panel_date));
        panel_rows.push(gecko_row(agg, "volume_24h_total", &panel_date));
    }
    panel_rows.push(jupiter_row(&jupiter, &panel_date));

    let panel_path = out_dir.join("depth_vs_volume_panel_publish.csv");
    write_depth_panel(&panel_path, &panel_rows)?;

    let aaplx = gecko_aggs
        .iter()
        .find(|a| a.symbol == "AAPLx")
        .context("AAPLx aggregate")?;
    let tslax = gecko_aggs.iter().find(|a| a.symbol == "TSLAx").unwrap();
    let spyx = gecko_aggs.iter().find(|a| a.symbol == "SPYx").unwrap();
    let impact_pct = jupiter.price_impact_pct.unwrap_or(0.0);

    let manifest = build_manifest(
        apr,
        jun,
        &bridged,
        aaplx,
        tslax,
        spyx,
        &jupiter,
        impact_pct,
        frozen_at,
        &panel_date,
        opts.live_apis,
        &out_dir,
    );

    write_json(&out_dir.join("manifest.json"), &manifest)?;

    println!("\nExchange evidence frozen to {}", out_dir.display());
    println!("  Panel: {}", panel_path.display());
    if opts.live_apis {
        print_live_validation(aaplx, tslax, spyx, impact_pct);
    } else {
        validate_publish_targets(
            aaplx,
            tslax,
            spyx,
            impact_pct,
            apr.monthly_transfer_volume_usd,
            jun.monthly_transfer_volume_usd,
            bridged.total_usd,
        )?;
        println!("\nPublish validation: all targets within tolerance.");
        for c in &manifest.claims {
            println!("  {} → {}", c.id, c.value_display);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_manifest(
    apr: &crate::exchange::rwa_xyz::PlatformSnapshot,
    jun: &crate::exchange::rwa_xyz::PlatformSnapshot,
    bridged: &crate::exchange::bridged::BridgedValueSum,
    aaplx: &SymbolPoolAggregate,
    tslax: &SymbolPoolAggregate,
    spyx: &SymbolPoolAggregate,
    jupiter: &JupiterQuoteEvidence,
    impact_pct: f64,
    frozen_at: String,
    panel_date: &str,
    live_apis: bool,
    out_dir: &std::path::Path,
) -> ExchangeManifest {
    let audit_id = if live_apis {
        config::exchange_live_audit_id(panel_date)
    } else {
        crate::core::bundle::EXCHANGE_BUNDLE.id.to_string()
    };
    let evidence = |file: &str| config::evidence_path_in_dir(out_dir, file);

    let mut manifest = AuditManifest::exchange_template(audit_id, frozen_at);
    manifest.panel_date = panel_date.into();
    manifest.claims = vec![
        ManifestClaim {
            id: "platform_transfer_apr_2026".into(),
            label: "RWA.xyz platform monthly transfer ~$1.03B (April 2026)".into(),
            value_display: format!("${:.2}B", apr.monthly_transfer_volume_usd / 1e9),
            value_usd: Some(apr.monthly_transfer_volume_usd),
            as_of: apr.date.clone(),
            evidence_file: evidence(SEED_TRANSFER_FILENAME),
            source_url: apr.source_url.clone(),
            caveat: "Peer-to-peer on-chain transfers; excludes mint/burn.".into(),
            status: None,
        },
        ManifestClaim {
            id: "platform_transfer_jun_2026".into(),
            label: "RWA.xyz platform monthly transfer ~$1.60B (June 2026)".into(),
            value_display: format!("${:.2}B", jun.monthly_transfer_volume_usd / 1e9),
            value_usd: Some(jun.monthly_transfer_volume_usd),
            as_of: jun.date.clone(),
            evidence_file: evidence(SEED_TRANSFER_FILENAME),
            source_url: jun.source_url.clone(),
            caveat: "Peer-to-peer on-chain transfers; excludes mint/burn.".into(),
            status: None,
        },
        ManifestClaim {
            id: "bridged_value_2026_06_11".into(),
            label: "Bridged / distributed value ~$764M".into(),
            value_display: format!("${:.0}M", bridged.total_usd / 1e6),
            value_usd: Some(bridged.total_usd),
            as_of: bridged.date.clone(),
            evidence_file: if live_apis {
                evidence("bridged_value_sum.json")
            } else {
                config::evidence_path_in_dir(
                    &config::exchange_publish_dir(),
                    "rwa-token-timeseries-export-1781314094816.csv",
                )
            },
            source_url: "https://app.rwa.xyz/platforms/xstocks".into(),
            caveat: if live_apis {
                "Live freeze bridged sum JSON beside manifest.".into()
            } else {
                "CSV export row sum; not transfer flow.".into()
            },
            status: None,
        },
        ManifestClaim {
            id: "aaplx_pool_tvl".into(),
            label: "AAPLx Solana pool TVL ~$124k".into(),
            value_display: format!("${:.0}k", aaplx.total_tvl_usd / 1e3),
            value_usd: Some(aaplx.total_tvl_usd),
            as_of: panel_date.into(),
            evidence_file: evidence("gecko_aaplx_pools.json"),
            source_url: aaplx.source_url.clone(),
            caveat: "GeckoTerminal Solana public pools aggregate.".into(),
            status: None,
        },
        ManifestClaim {
            id: "aaplx_pool_vol_24h".into(),
            label: "AAPLx Solana 24h volume ~$35k".into(),
            value_display: format!("${:.0}k", aaplx.total_24h_vol_usd / 1e3),
            value_usd: Some(aaplx.total_24h_vol_usd),
            as_of: panel_date.into(),
            evidence_file: evidence("gecko_aaplx_pools.json"),
            source_url: aaplx.source_url.clone(),
            caveat: "GeckoTerminal Solana public pools aggregate.".into(),
            status: None,
        },
        ManifestClaim {
            id: "jupiter_aaplx_100k_impact".into(),
            label: "Jupiter AAPLx @ $100k USDC price impact".into(),
            value_display: format!("{impact_pct:.0}%"),
            value_usd: None,
            as_of: panel_date.into(),
            evidence_file: evidence("jupiter_quote_aaplx_100k.json"),
            source_url: jupiter.source_url.clone(),
            caveat: "Quote-only; publish fixture or live API at freeze time.".into(),
            status: None,
        },
        ManifestClaim {
            id: "spyx_pool_vol_24h_fig4".into(),
            label: "Fig. 4 — SPYx 24h pool volume".into(),
            value_display: format!("${:.1}M", spyx.total_24h_vol_usd / 1e6),
            value_usd: Some(spyx.total_24h_vol_usd),
            as_of: panel_date.into(),
            evidence_file: evidence("gecko_spyx_pools.json"),
            source_url: spyx.source_url.clone(),
            caveat: "Fig. 4 input; not in prose body.".into(),
            status: None,
        },
        ManifestClaim {
            id: "tslax_pool_vol_24h_fig4".into(),
            label: "Fig. 4 — TSLAx 24h pool volume".into(),
            value_display: format!("${:.1}M", tslax.total_24h_vol_usd / 1e6),
            value_usd: Some(tslax.total_24h_vol_usd),
            as_of: panel_date.into(),
            evidence_file: evidence("gecko_tslax_pools.json"),
            source_url: tslax.source_url.clone(),
            caveat: "Fig. 4 input; not in prose body.".into(),
            status: None,
        },
    ];
    manifest
}

fn frozen_at_timestamp() -> String {
    std::env::var("RWA_AUDIT_FROZEN_AT").unwrap_or_else(|_| Utc::now().to_rfc3339())
}

fn validate_publish_targets(
    aaplx: &SymbolPoolAggregate,
    tslax: &SymbolPoolAggregate,
    spyx: &SymbolPoolAggregate,
    impact_pct: f64,
    apr_usd: f64,
    jun_usd: f64,
    bridged_usd: f64,
) -> Result<()> {
    assert_near(apr_usd, 1.03e9, 0.02e9, "platform transfer April")?;
    assert_near(jun_usd, 1.60e9, 0.02e9, "platform transfer June")?;
    assert_near(bridged_usd, 763.76e6, 1.0e6, "bridged value")?;
    assert_near(aaplx.total_tvl_usd, 124_062.35, 500.0, "AAPLx TVL")?;
    assert_near(aaplx.total_24h_vol_usd, 34_771.14, 500.0, "AAPLx 24h vol")?;
    assert_near(
        tslax.total_24h_vol_usd,
        3_358_518.82,
        50_000.0,
        "TSLAx 24h vol",
    )?;
    assert_near(
        spyx.total_24h_vol_usd,
        7_102_758.23,
        50_000.0,
        "SPYx 24h vol",
    )?;
    assert_near(impact_pct, 68.2, 2.0, "Jupiter $100k impact")?;
    Ok(())
}

fn assert_near(actual: f64, expected: f64, tol: f64, label: &str) -> Result<()> {
    if (actual - expected).abs() > tol {
        anyhow::bail!("{label}: got {actual}, expected {expected} ± {tol}");
    }
    Ok(())
}

fn print_live_validation(
    aaplx: &SymbolPoolAggregate,
    tslax: &SymbolPoolAggregate,
    spyx: &SymbolPoolAggregate,
    impact_pct: f64,
) {
    println!("\nLive API snapshot (may differ from published post):");
    println!(
        "  AAPLx TVL ${:.0}k (post ~$124k), vol ${:.0}k (post ~$35k)",
        aaplx.total_tvl_usd / 1e3,
        aaplx.total_24h_vol_usd / 1e3
    );
    println!(
        "  TSLAx vol ${:.1}M (post ~$3.4M), SPYx vol ${:.1}M (post ~$7.1M)",
        tslax.total_24h_vol_usd / 1e6,
        spyx.total_24h_vol_usd / 1e6
    );
    println!("  Jupiter impact {impact_pct:.1}% (post cites ~68%)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_freeze_rejects_mismatched_publish_date() {
        let err = freeze_exchange_evidence(ExchangeFreezeOptions {
            live_apis: false,
            refresh_rwa_xyz: false,
            panel_date: Some("2026-06-15".into()),
        })
        .unwrap_err();
        assert!(err.to_string().contains("offline freeze only supports"));
    }

    #[test]
    fn live_manifest_uses_staging_evidence_paths() {
        let out_dir = config::exchange_live_staging_dir();
        let manifest = build_manifest(
            &crate::exchange::rwa_xyz::PlatformSnapshot {
                date: "2026-04-20".into(),
                monthly_transfer_volume_usd: 1.03e9,
                source_url: "u".into(),
                accessed_date: "2026-06-18".into(),
                confidence: "high".into(),
                caveat: "c".into(),
                source_type: None,
                exclude_from_interpolation: None,
            },
            &crate::exchange::rwa_xyz::PlatformSnapshot {
                date: "2026-06-12".into(),
                monthly_transfer_volume_usd: 1.6e9,
                source_url: "u".into(),
                accessed_date: "2026-06-18".into(),
                confidence: "high".into(),
                caveat: "c".into(),
                source_type: None,
                exclude_from_interpolation: None,
            },
            &crate::exchange::bridged::BridgedValueSum {
                date: "2026-06-11".into(),
                total_usd: 763.76e6,
                source_file: "fixtures.csv".into(),
            },
            &SymbolPoolAggregate {
                symbol: "AAPLx".into(),
                total_tvl_usd: 124_062.35,
                total_24h_vol_usd: 34_771.14,
                source_url: "u".into(),
                pool_count: 1,
                top_pool_vol_share: None,
            },
            &SymbolPoolAggregate {
                symbol: "TSLAx".into(),
                total_tvl_usd: 1.0,
                total_24h_vol_usd: 3.4e6,
                source_url: "u".into(),
                pool_count: 1,
                top_pool_vol_share: None,
            },
            &SymbolPoolAggregate {
                symbol: "SPYx".into(),
                total_tvl_usd: 1.0,
                total_24h_vol_usd: 7.1e6,
                source_url: "u".into(),
                pool_count: 1,
                top_pool_vol_share: None,
            },
            &JupiterQuoteEvidence {
                input_mint: "a".into(),
                output_mint: "b".into(),
                input_symbol: "USDC".into(),
                output_symbol: "AAPLx".into(),
                input_amount_usd: 100_000,
                input_amount_raw: 100_000_000_000,
                slippage_bps: 100,
                price_impact_pct: Some(68.0),
                out_amount_raw: None,
                route_labels: vec![],
                source_url: "u".into(),
                raw_response: serde_json::json!({}),
            },
            68.0,
            "2026-06-18T00:00:00Z".into(),
            "2026-06-18",
            true,
            &out_dir,
        );
        assert_eq!(
            manifest.audit_id.as_deref(),
            Some("exchange-live-2026-06-18")
        );
        assert!(manifest.claims[0]
            .evidence_file
            .starts_with("data/exchange-live/"));
    }

    #[test]
    fn materialize_live_transfer_seed_without_refresh_uses_temp_dir() {
        let publish_seed = config::platform_seed_path();
        if !publish_seed.exists() {
            return;
        }
        let out_dir = std::env::temp_dir().join(format!(
            "rwa-live-seed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&out_dir).unwrap();
        let seed = load_seed().unwrap();

        materialize_transfer_seed_for_live_staging(true, false, &out_dir, &seed).unwrap();
        assert!(out_dir.join(SEED_TRANSFER_FILENAME).exists());

        materialize_transfer_seed_for_live_staging(true, true, &out_dir, &seed).unwrap();
        materialize_transfer_seed_for_live_staging(false, false, &out_dir, &seed).unwrap();

        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    fn frozen_at_timestamp_uses_env_when_set() {
        std::env::set_var("RWA_AUDIT_FROZEN_AT", "2026-01-01T00:00:00Z");
        assert_eq!(frozen_at_timestamp(), "2026-01-01T00:00:00Z");
        std::env::remove_var("RWA_AUDIT_FROZEN_AT");
    }
}
