use std::collections::HashMap;

use anyhow::Result;
use chrono::NaiveDate;

use crate::config::ensure_dir;
use crate::flow::config::{
    flow_data_dir, panel_end_date, panel_start_date, PANEL_TOKENS, MIN_POOL_VOLUME_USD,
    MAX_POOLS_PER_TOKEN,
};
use crate::flow::gecko::{GeckoClient, PoolMeta};
use crate::flow::output::{
    write_panel_daily, write_panel_pool_detail, write_panel_summary, write_reference_gold,
    PanelDailyRow, PanelPoolRow, PanelSummary,
};
use crate::flow::reference::fetch_gc_futures;
use crate::flow::stats::{coefficient_of_variation, pearson_r, robust_z, spike_ratio};

#[derive(Debug, Clone)]
struct DayAgg {
    total_volume: f64,
    pool_volumes: Vec<(String, f64)>,
}

pub fn collect_flow_panel() -> Result<()> {
    let out_dir = flow_data_dir();
    ensure_dir(&out_dir)?;

    let start = panel_start_date();
    let end = panel_end_date();
    let gecko = GeckoClient::new()?;

    let gold = fetch_gc_futures(start, end)?;
    write_reference_gold(&out_dir, &gold)?;
    let gold_abs: HashMap<NaiveDate, f64> = gold.iter().map(|g| (g.date, g.abs_return)).collect();
    let gold_z_map: HashMap<NaiveDate, f64> = {
        let dates: Vec<_> = gold.iter().map(|g| g.date).collect();
        let vals: Vec<f64> = gold.iter().map(|g| g.abs_return).collect();
        let zs = robust_z(&vals);
        dates.into_iter().zip(zs).collect()
    };

    let mut panel_rows = Vec::new();
    let mut pool_rows = Vec::new();
    let mut summaries = Vec::new();

    for token in PANEL_TOKENS {
        println!("Fetching GeckoTerminal pools for {}...", token.symbol);
        let pools = gecko.token_pools(token.address)?;
        println!("  {} pools listed", pools.len());

        let active_pools: Vec<&PoolMeta> = pools
            .iter()
            .filter(|p| p.volume_h24_usd > 0.0 || p.reserve_usd > 100.0)
            .take(MAX_POOLS_PER_TOKEN)
            .collect();

        let mut daily: HashMap<NaiveDate, DayAgg> = HashMap::new();

        for pool in &active_pools {
            println!("  OHLCV {} ({})", pool.address, pool.name);
            let ohlcv = gecko.pool_daily_ohlcv(&pool.address, 120)?;
            for bar in ohlcv {
                if bar.date < start || bar.date > end {
                    continue;
                }
                pool_rows.push(PanelPoolRow {
                    date: bar.date.to_string(),
                    symbol: token.symbol.to_string(),
                    pool_address: pool.address.clone(),
                    pool_name: pool.name.clone(),
                    volume_usd: bar.volume_usd,
                    reserve_usd_snapshot: pool.reserve_usd,
                });
                let agg = daily.entry(bar.date).or_insert_with(|| DayAgg {
                    total_volume: 0.0,
                    pool_volumes: Vec::new(),
                });
                agg.total_volume += bar.volume_usd;
                agg.pool_volumes.push((pool.address.clone(), bar.volume_usd));
            }
        }

        let dates: Vec<NaiveDate> = (0..=(end - start).num_days())
            .map(|d| start + chrono::Duration::days(d))
            .collect();

        let mut raw_volumes = Vec::new();
        let mut raw_top_shares = Vec::new();
        let mut raw_dispersions = Vec::new();

        for date in &dates {
            let agg = daily.get(date);
            let total = agg.map(|a| a.total_volume).unwrap_or(0.0);
            let _active_pool_count = agg
                .map(|a| a.pool_volumes.iter().filter(|(_, v)| *v >= MIN_POOL_VOLUME_USD).count())
                .unwrap_or(0);
            let top_vol = agg
                .and_then(|a| a.pool_volumes.iter().map(|(_, v)| *v).max_by(|a, b| a.partial_cmp(b).unwrap()))
                .unwrap_or(0.0);
            let top_share = if total > 0.0 { top_vol / total } else { 0.0 };
            let dispersion = 1.0 - top_share;

            raw_volumes.push(total);
            raw_top_shares.push(top_share);
            raw_dispersions.push(dispersion);
        }

        let vol_z = robust_z(&raw_volumes);
        let share_z = robust_z(&raw_top_shares);
        let disp_z = robust_z(&raw_dispersions);

        for (i, date) in dates.iter().enumerate() {
            let total = raw_volumes[i];
            let top_share = raw_top_shares[i];
            let dispersion = raw_dispersions[i];
            let active_pool_count = daily
                .get(date)
                .map(|a| a.pool_volumes.iter().filter(|(_, v)| *v * 1.0 >= MIN_POOL_VOLUME_USD).count())
                .unwrap_or(0);

            panel_rows.push(PanelDailyRow {
                date: date.to_string(),
                symbol: token.symbol.to_string(),
                total_volume_usd: total,
                active_pool_count: active_pool_count as u32,
                top_pool_volume_share: top_share,
                routing_dispersion: dispersion,
                volume_robust_z: vol_z[i],
                top_pool_share_robust_z: share_z[i],
                routing_dispersion_robust_z: disp_z[i],
                gold_abs_return: gold_abs.get(date).copied(),
                gold_abs_return_robust_z: gold_z_map.get(date).copied(),
            });
        }

        let active_days = raw_volumes.iter().filter(|v| **v > 0.0).count();
        let total_days = dates.len();
        let median_vol = {
            let mut nz: Vec<f64> = raw_volumes.iter().copied().filter(|v| *v > 0.0).collect();
            nz.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if nz.is_empty() { 0.0 } else { nz[nz.len() / 2] }
        };
        let median_top_share = {
            let mut s = raw_top_shares.clone();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            s[s.len() / 2]
        };
        let near_one_share_days = raw_top_shares.iter().filter(|s| **s >= 0.99).count();

        let r_gold_disp = if token.symbol == "PAXG" {
            let mut gold_z_aligned = Vec::new();
            let mut disp_z_aligned = Vec::new();
            for (i, date) in dates.iter().enumerate() {
                if let Some(gz) = gold_z_map.get(date) {
                    gold_z_aligned.push(*gz);
                    disp_z_aligned.push(disp_z[i]);
                }
            }
            pearson_r(&gold_z_aligned, &disp_z_aligned)
        } else {
            None
        };
        let r_vol_share = pearson_r(&vol_z, &share_z);

        summaries.push(PanelSummary {
            symbol: token.symbol.to_string(),
            panel_start: start.to_string(),
            panel_end: end.to_string(),
            total_days: total_days as u32,
            active_volume_days: active_days as u32,
            pool_count_listed: pools.len() as u32,
            median_daily_volume_usd: median_vol,
            median_top_pool_volume_share: median_top_share,
            days_top_share_at_or_above_99pct: near_one_share_days as u32,
            volume_cv: coefficient_of_variation(&raw_volumes),
            volume_spike_ratio: spike_ratio(&raw_volumes),
            corr_gold_z_vs_routing_dispersion_z: r_gold_disp,
            corr_volume_z_vs_top_share_z: r_vol_share,
        });
    }

    write_panel_pool_detail(&out_dir, &pool_rows)?;
    write_panel_daily(&out_dir, &panel_rows)?;
    write_panel_summary(&out_dir, &summaries)?;

    println!("Wrote panel data to {}", out_dir.display());
    Ok(())
}
