use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::blocking::Client;

use crate::config::ensure_dir;
use crate::exchange::bridged::sum_bridged_value_for_date;
use crate::exchange::config::{
    self, BRIDGED_VALUE_DATE, PLATFORM_TRANSFER_APR_DATE, PLATFORM_TRANSFER_JUN_DATE,
    PUBLISH_PANEL_DATE, XSTOCKS_SOLANA,
};
use crate::exchange::output::{
    bridged_row, gecko_row, jupiter_row, platform_row, write_depth_panel, write_json, DepthPanelRow,
    ExchangeManifest, ManifestClaim,
};
use crate::exchange::reference::{gecko_aggregate_from_reference, jupiter_quote_from_publish_fixture};
use crate::exchange::rwa_xyz::{fetch_and_merge_seed, load_seed, snapshot_for_date};
use crate::flow::config::JUPITER_QUOTE_USD;
use crate::flow::gecko::{fetch_solana_symbol_pool_aggregate, SymbolPoolAggregate};
use crate::flow::jupiter::{fetch_aaplx_quote_100k, JupiterQuoteEvidence};

#[derive(Debug, Clone, Copy)]
pub struct ExchangeFreezeOptions {
    pub refresh_rwa_xyz: bool,
    /// Fetch Gecko + Jupiter live APIs. Default uses regression panel + publish fixture.
    pub live_apis: bool,
}

impl Default for ExchangeFreezeOptions {
    fn default() -> Self {
        Self {
            refresh_rwa_xyz: false,
            live_apis: false,
        }
    }
}

pub fn freeze_exchange_evidence(opts: ExchangeFreezeOptions) -> Result<()> {
    let out_dir = config::exchange_data_dir();
    ensure_dir(&out_dir)?;

    let accessed = if opts.live_apis {
        Utc::now().format("%Y-%m-%d").to_string()
    } else {
        PUBLISH_PANEL_DATE.into()
    };
    let frozen_at = Utc::now().to_rfc3339();

    let seed = if opts.refresh_rwa_xyz {
        println!("Fetching RWA.xyz platform snapshots...");
        fetch_and_merge_seed(&accessed)?
    } else {
        load_seed().context("load rwa_xyz seed — run with --refresh-rwa or copy seeds/")?
    };

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
            write_json(&out_dir.join(&fname), &agg)?;
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
        write_json(&out_dir.join("jupiter_quote_aaplx_100k.json"), &q)?;
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
        panel_rows.push(gecko_row(agg, "pool_tvl_total", PUBLISH_PANEL_DATE));
        panel_rows.push(gecko_row(agg, "volume_24h_total", PUBLISH_PANEL_DATE));
    }
    panel_rows.push(jupiter_row(&jupiter, PUBLISH_PANEL_DATE));

    let panel_path = config::publish_panel_path();
    write_depth_panel(&panel_path, &panel_rows)?;

    let aaplx = gecko_aggs
        .iter()
        .find(|a| a.symbol == "AAPLx")
        .context("AAPLx aggregate")?;
    let tslax = gecko_aggs.iter().find(|a| a.symbol == "TSLAx").unwrap();
    let spyx = gecko_aggs.iter().find(|a| a.symbol == "SPYx").unwrap();
    let impact_pct = jupiter.price_impact_pct.unwrap_or(0.0);

    let manifest = build_manifest(apr, jun, &bridged, aaplx, tslax, spyx, &jupiter, impact_pct, frozen_at);

    write_json(&config::manifest_path(), &manifest)?;

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
) -> ExchangeManifest {
    ExchangeManifest {
        article: "Part III — Where RWA Exchange Risk Actually Sits".into(),
        post_url: "https://egpivo.github.io/2026/06/21/where-rwa-exchange-risk-actually-sits.html".into(),
        frozen_at,
        panel_date: PUBLISH_PANEL_DATE.into(),
        do_not_claim: vec![
            "Platform transfer ≠ CEX trading volume".into(),
            "Bridged value ≠ transfer volume".into(),
            "Jupiter quote ≠ executed trade or exit capacity".into(),
            "Do not publish rwa_xyz monthly_interpolated extrapolation rows".into(),
            "Do not use exclude_from_interpolation suspect headline ($21.9M)".into(),
        ],
        claims: vec![
            ManifestClaim {
                id: "platform_transfer_apr_2026".into(),
                label: "RWA.xyz platform monthly transfer ~$1.03B (April 2026)".into(),
                value_display: format!("${:.2}B", apr.monthly_transfer_volume_usd / 1e9),
                value_usd: Some(apr.monthly_transfer_volume_usd),
                as_of: apr.date.clone(),
                evidence_file: "artifacts/data/rwa_xyz_platform_transfer_snapshots.json".into(),
                source_url: apr.source_url.clone(),
                caveat: "Peer-to-peer on-chain transfers; excludes mint/burn.".into(),
            },
            ManifestClaim {
                id: "platform_transfer_jun_2026".into(),
                label: "RWA.xyz platform monthly transfer ~$1.60B (June 2026)".into(),
                value_display: format!("${:.2}B", jun.monthly_transfer_volume_usd / 1e9),
                value_usd: Some(jun.monthly_transfer_volume_usd),
                as_of: jun.date.clone(),
                evidence_file: "artifacts/data/rwa_xyz_platform_transfer_snapshots.json".into(),
                source_url: jun.source_url.clone(),
                caveat: "Peer-to-peer on-chain transfers; excludes mint/burn.".into(),
            },
            ManifestClaim {
                id: "bridged_value_2026_06_11".into(),
                label: "Bridged / distributed value ~$764M".into(),
                value_display: format!("${:.0}M", bridged.total_usd / 1e6),
                value_usd: Some(bridged.total_usd),
                as_of: bridged.date.clone(),
                evidence_file: "artifacts/data/rwa-token-timeseries-export-1781314094816.csv".into(),
                source_url: "https://app.rwa.xyz/platforms/xstocks".into(),
                caveat: "CSV export row sum; not transfer flow.".into(),
            },
            ManifestClaim {
                id: "aaplx_pool_tvl".into(),
                label: "AAPLx Solana pool TVL ~$124k".into(),
                value_display: format!("${:.0}k", aaplx.total_tvl_usd / 1e3),
                value_usd: Some(aaplx.total_tvl_usd),
                as_of: PUBLISH_PANEL_DATE.into(),
                evidence_file: "artifacts/data/gecko_aaplx_pools.json".into(),
                source_url: aaplx.source_url.clone(),
                caveat: "GeckoTerminal Solana public pools aggregate.".into(),
            },
            ManifestClaim {
                id: "aaplx_pool_vol_24h".into(),
                label: "AAPLx Solana 24h volume ~$35k".into(),
                value_display: format!("${:.0}k", aaplx.total_24h_vol_usd / 1e3),
                value_usd: Some(aaplx.total_24h_vol_usd),
                as_of: PUBLISH_PANEL_DATE.into(),
                evidence_file: "artifacts/data/gecko_aaplx_pools.json".into(),
                source_url: aaplx.source_url.clone(),
                caveat: "GeckoTerminal Solana public pools aggregate.".into(),
            },
            ManifestClaim {
                id: "jupiter_aaplx_100k_impact".into(),
                label: "Jupiter AAPLx @ $100k USDC price impact".into(),
                value_display: format!("{impact_pct:.0}%"),
                value_usd: None,
                as_of: PUBLISH_PANEL_DATE.into(),
                evidence_file: "artifacts/data/jupiter_quote_aaplx_100k.json".into(),
                source_url: jupiter.source_url.clone(),
                caveat: "Quote-only; publish fixture or live API at freeze time.".into(),
            },
            ManifestClaim {
                id: "spyx_pool_vol_24h_fig4".into(),
                label: "Fig. 4 — SPYx 24h pool volume".into(),
                value_display: format!("${:.1}M", spyx.total_24h_vol_usd / 1e6),
                value_usd: Some(spyx.total_24h_vol_usd),
                as_of: PUBLISH_PANEL_DATE.into(),
                evidence_file: "artifacts/data/gecko_spyx_pools.json".into(),
                source_url: spyx.source_url.clone(),
                caveat: "Fig. 4 input; not in prose body.".into(),
            },
            ManifestClaim {
                id: "tslax_pool_vol_24h_fig4".into(),
                label: "Fig. 4 — TSLAx 24h pool volume".into(),
                value_display: format!("${:.1}M", tslax.total_24h_vol_usd / 1e6),
                value_usd: Some(tslax.total_24h_vol_usd),
                as_of: PUBLISH_PANEL_DATE.into(),
                evidence_file: "artifacts/data/gecko_tslax_pools.json".into(),
                source_url: tslax.source_url.clone(),
                caveat: "Fig. 4 input; not in prose body.".into(),
            },
        ],
    }
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
    assert_near(tslax.total_24h_vol_usd, 3_358_518.82, 50_000.0, "TSLAx 24h vol")?;
    assert_near(spyx.total_24h_vol_usd, 7_102_758.23, 50_000.0, "SPYx 24h vol")?;
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
