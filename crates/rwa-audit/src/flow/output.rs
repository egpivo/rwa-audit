use std::fs::File;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct PanelPoolRow {
    pub date: String,
    pub symbol: String,
    pub pool_address: String,
    pub pool_name: String,
    pub volume_usd: f64,
    pub reserve_usd_snapshot: f64,
}

#[derive(Debug, Serialize)]
pub struct PanelDailyRow {
    pub date: String,
    pub symbol: String,
    pub total_volume_usd: f64,
    pub active_pool_count: u32,
    pub top_pool_volume_share: f64,
    pub routing_dispersion: f64,
    pub volume_robust_z: f64,
    pub top_pool_share_robust_z: f64,
    pub routing_dispersion_robust_z: f64,
    pub gold_abs_return: Option<f64>,
    pub gold_abs_return_robust_z: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PanelSummary {
    pub symbol: String,
    pub panel_start: String,
    pub panel_end: String,
    pub total_days: u32,
    pub active_volume_days: u32,
    pub pool_count_listed: u32,
    pub median_daily_volume_usd: f64,
    pub median_top_pool_volume_share: f64,
    pub days_top_share_at_or_above_99pct: u32,
    pub volume_cv: Option<f64>,
    pub volume_spike_ratio: Option<f64>,
    pub corr_gold_z_vs_routing_dispersion_z: Option<f64>,
    pub corr_volume_z_vs_top_share_z: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ParaswapQuoteRow {
    pub checkpoint_date: String,
    pub symbol: String,
    pub src_token: String,
    pub dest_token: String,
    pub amount_usd: u64,
    pub route_found: bool,
    pub dest_amount_usdc: Option<f64>,
    pub route_summary: String,
    pub error_message: Option<String>,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct TxReconRow {
    pub label: String,
    pub tx_hash: String,
    pub block_number: u64,
    pub log_count: u32,
    pub transfer_count: u32,
    pub mint_count: u32,
    pub burn_count: u32,
    pub swap_count: u32,
    pub unique_transfer_recipients: u32,
    pub distinct_log_contracts: u32,
    pub log_summaries: Vec<String>,
    pub source: String,
}

pub fn write_panel_pool_detail(dir: &Path, rows: &[PanelPoolRow]) -> anyhow::Result<()> {
    let path = dir.join("pool_daily_detail.csv");
    let mut wtr = csv::Writer::from_path(&path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    println!("  Wrote {}", path.display());
    Ok(())
}

pub fn write_panel_daily(dir: &Path, rows: &[PanelDailyRow]) -> anyhow::Result<()> {
    let path = dir.join("panel_daily.csv");
    let mut wtr = csv::Writer::from_path(&path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    println!("  Wrote {}", path.display());
    Ok(())
}

pub fn write_panel_summary(dir: &Path, summaries: &[PanelSummary]) -> anyhow::Result<()> {
    let json_path = dir.join("panel_summary.json");
    let json = serde_json::to_string_pretty(summaries)?;
    std::fs::write(&json_path, json)?;
    println!("  Wrote {}", json_path.display());

    let md_path = dir.join("panel_summary.md");
    let mut f = File::create(&md_path)?;
    writeln!(f, "# Flow panel summary\n")?;
    for s in summaries {
        writeln!(f, "## {}\n", s.symbol)?;
        writeln!(f, "- Window: {} → {}", s.panel_start, s.panel_end)?;
        writeln!(f, "- Active volume days: {}/{}", s.active_volume_days, s.total_days)?;
        writeln!(f, "- Pools listed (GeckoTerminal): {}", s.pool_count_listed)?;
        writeln!(
            f,
            "- Median daily volume (USD): {:.2}",
            s.median_daily_volume_usd
        )?;
        writeln!(
            f,
            "- Median top-pool volume share: {:.2}%",
            s.median_top_pool_volume_share * 100.0
        )?;
        writeln!(
            f,
            "- Days with top-pool share ≥ 99%: {}",
            s.days_top_share_at_or_above_99pct
        )?;
        if let Some(cv) = s.volume_cv {
            writeln!(f, "- Volume CV: {cv:.2}")?;
        }
        if let Some(sr) = s.volume_spike_ratio {
            writeln!(f, "- Volume spike ratio (max/median): {sr:.1}×")?;
        }
        if let Some(r) = s.corr_gold_z_vs_routing_dispersion_z {
            writeln!(f, "- r(gold abs-return z, routing dispersion z): {r:.2}")?;
        }
        if let Some(r) = s.corr_volume_z_vs_top_share_z {
            writeln!(f, "- r(volume z, top-pool share z): {r:.2}")?;
        }
        writeln!(f)?;
    }
    println!("  Wrote {}", md_path.display());
    Ok(())
}

pub fn write_paraswap_quotes(dir: &Path, rows: &[ParaswapQuoteRow]) -> anyhow::Result<()> {
    let path = dir.join("paraswap_quotes.csv");
    let mut wtr = csv::Writer::from_path(&path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn write_tx_reconstructions(dir: &Path, rows: &[TxReconRow]) -> anyhow::Result<()> {
    let path = dir.join("tx_reconstructions.json");
    let json = serde_json::to_string_pretty(rows)?;
    std::fs::write(&path, json)?;
    Ok(())
}

pub fn write_reference_gold(dir: &Path, rows: &[crate::flow::reference::GoldDaily]) -> anyhow::Result<()> {
    let path = dir.join("reference_gc.csv");
    let mut wtr = csv::Writer::from_path(&path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    println!("  Wrote {}", path.display());
    Ok(())
}
