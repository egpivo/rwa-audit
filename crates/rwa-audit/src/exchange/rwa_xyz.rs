use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::exchange::config::{platform_seed_path, RWA_XSTOCKS_URL};

pub const SEED_TRANSFER_FILENAME: &str = "rwa_xyz_platform_transfer_snapshots.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSnapshot {
    pub date: String,
    pub monthly_transfer_volume_usd: f64,
    pub source_url: String,
    pub accessed_date: String,
    pub confidence: String,
    pub caveat: String,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub exclude_from_interpolation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSeedFile {
    pub metric: String,
    pub definition_url: String,
    pub note: String,
    #[serde(default)]
    pub last_fetched: Option<String>,
    pub snapshots: Vec<PlatformSnapshot>,
}

pub fn seed_path() -> PathBuf {
    platform_seed_path()
}

pub fn load_seed() -> Result<PlatformSeedFile> {
    load_seed_from(&seed_path())
}

pub fn load_seed_from(path: &Path) -> Result<PlatformSeedFile> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read RWA.xyz seed {}", path.display()))?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_seed_to(path: &Path, seed: &PlatformSeedFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(seed)? + "\n")
        .with_context(|| format!("write RWA.xyz seed {}", path.display()))?;
    Ok(())
}

pub fn save_seed(seed: &PlatformSeedFile) -> Result<()> {
    save_seed_to(&seed_path(), seed)
}

pub fn snapshot_for_date<'a>(
    seed: &'a PlatformSeedFile,
    date: &str,
) -> Option<&'a PlatformSnapshot> {
    seed.snapshots
        .iter()
        .filter(|s| !s.exclude_from_interpolation.unwrap_or(false))
        .filter(|s| {
            s.confidence != "low" || s.source_type.as_deref() != Some("rwa_ssr_headline_suspect")
        })
        .find(|s| s.date == date)
}

/// Fetch RWA.xyz SSR snapshots and persist merged seed to `save_path` only.
pub fn fetch_and_merge_seed(access_date: &str, save_path: &Path) -> Result<PlatformSeedFile> {
    let mut seed = load_seed_from(&seed_path()).unwrap_or(PlatformSeedFile {
        metric: "monthly_transfer_volume".into(),
        definition_url: "https://docs.rwa.xyz/methodology/metrics".into(),
        note: "Platform transfer snapshots; not trading volume.".into(),
        last_fetched: None,
        snapshots: vec![],
    });

    let html = reqwest::blocking::Client::builder()
        .user_agent("rwa-audit/0.1")
        .build()?
        .get(RWA_XSTOCKS_URL)
        .send()?
        .error_for_status()?
        .text()?;

    let json_str = extract_next_data(&html)?;
    let page: Value = serde_json::from_str(&json_str)?;
    let platform = &page["props"]["pageProps"]["platform"];
    let trailing = platform
        .get("trailing_30_day_transfer_volume")
        .cloned()
        .unwrap_or(Value::Null);

    let mut scraped = lag_snapshots(access_date, &trailing);
    if let Some(headline) = headline_snapshot(
        access_date,
        &page,
        trailing.get("val_7d").and_then(|v| v.as_f64()),
    ) {
        scraped.push(headline);
    }

    seed.snapshots = merge_snapshots(&seed.snapshots, &scraped);
    seed.last_fetched = Some(access_date.to_string());
    save_seed_to(save_path, &seed)?;
    Ok(seed)
}

fn extract_next_data(html: &str) -> Result<String> {
    let marker = r#"<script id="__NEXT_DATA__" type="application/json">"#;
    let start = html.find(marker).context("missing __NEXT_DATA__")? + marker.len();
    let end = html[start..]
        .find("</script>")
        .context("unclosed __NEXT_DATA__")?
        + start;
    Ok(html[start..end].to_string())
}

fn lag_snapshots(access_date: &str, trailing: &Value) -> Vec<PlatformSnapshot> {
    let base = chrono::NaiveDate::parse_from_str(access_date, "%Y-%m-%d").ok();
    let Some(base) = base else {
        return vec![];
    };
    let mut out = vec![];
    for (days, key, label) in [
        (7i64, "val_7d", "trailing_30d_transfer_volume lag 7d"),
        (30, "val_30d", "trailing_30d_transfer_volume lag 30d"),
        (90, "val_90d", "trailing_30d_transfer_volume lag 90d"),
    ] {
        let val = trailing.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0);
        if val <= 0.0 {
            continue;
        }
        let snap_date = (base - chrono::Duration::days(days)).to_string();
        out.push(PlatformSnapshot {
            date: snap_date,
            monthly_transfer_volume_usd: (val * 100.0).round() / 100.0,
            source_url: RWA_XSTOCKS_URL.into(),
            accessed_date: access_date.into(),
            confidence: "high".into(),
            caveat: format!(
                "RWA.xyz SSR {label}; trailing 30-day on-chain transfer USD (excl. mint/burn)."
            ),
            source_type: Some("rwa_ssr_trailing_30d_lag".into()),
            exclude_from_interpolation: None,
        });
    }
    out
}

fn headline_snapshot(
    access_date: &str,
    page: &Value,
    trailing_7d: Option<f64>,
) -> Option<PlatformSnapshot> {
    let aggregates = page["props"]["pageProps"]["aggregates"].as_array()?;
    let headline = aggregates
        .iter()
        .find(|a| a.get("label").and_then(|l| l.as_str()) == Some("Monthly Transfer Volume"))?;
    let val = headline.get("value")?.as_f64()?;
    if let Some(t7) = trailing_7d {
        if t7 > 0.0 && val < t7 * 0.05 {
            return Some(PlatformSnapshot {
                date: access_date.into(),
                monthly_transfer_volume_usd: (val * 100.0).round() / 100.0,
                source_url: RWA_XSTOCKS_URL.into(),
                accessed_date: access_date.into(),
                confidence: "low".into(),
                caveat: format!(
                    "Dashboard headline Monthly Transfer Volume; partial-month artifact vs trailing_30d ${:.2}B. Not for publish.",
                    t7 / 1e9
                ),
                source_type: Some("rwa_ssr_headline_suspect".into()),
                exclude_from_interpolation: Some(true),
            });
        }
    }
    Some(PlatformSnapshot {
        date: access_date.into(),
        monthly_transfer_volume_usd: (val * 100.0).round() / 100.0,
        source_url: RWA_XSTOCKS_URL.into(),
        accessed_date: access_date.into(),
        confidence: "high".into(),
        caveat: "RWA.xyz SSR dashboard aggregate Monthly Transfer Volume".into(),
        source_type: Some("rwa_ssr_headline".into()),
        exclude_from_interpolation: None,
    })
}

fn merge_snapshots(
    existing: &[PlatformSnapshot],
    new_rows: &[PlatformSnapshot],
) -> Vec<PlatformSnapshot> {
    let mut by_date: std::collections::BTreeMap<String, PlatformSnapshot> =
        std::collections::BTreeMap::new();
    let rank = |c: &str| match c {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    };
    for row in existing.iter().chain(new_rows.iter()) {
        let d = row.date.clone();
        match by_date.get(&d) {
            None => {
                by_date.insert(d, row.clone());
            }
            Some(prev) if rank(&row.confidence) > rank(&prev.confidence) => {
                by_date.insert(d, row.clone());
            }
            _ => {}
        }
    }
    by_date.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_for_date_skips_excluded_headline() {
        let seed = PlatformSeedFile {
            metric: "x".into(),
            definition_url: "x".into(),
            note: "x".into(),
            last_fetched: None,
            snapshots: vec![
                PlatformSnapshot {
                    date: "2026-06-12".into(),
                    monthly_transfer_volume_usd: 1.6e9,
                    source_url: "u".into(),
                    accessed_date: "2026-06-12".into(),
                    confidence: "high".into(),
                    caveat: "ok".into(),
                    source_type: None,
                    exclude_from_interpolation: None,
                },
                PlatformSnapshot {
                    date: "2026-06-13".into(),
                    monthly_transfer_volume_usd: 21.9e6,
                    source_url: "u".into(),
                    accessed_date: "2026-06-13".into(),
                    confidence: "low".into(),
                    caveat: "bad".into(),
                    source_type: Some("rwa_ssr_headline_suspect".into()),
                    exclude_from_interpolation: Some(true),
                },
            ],
        };
        assert_eq!(
            snapshot_for_date(&seed, "2026-06-12")
                .unwrap()
                .monthly_transfer_volume_usd,
            1.6e9
        );
        assert!(snapshot_for_date(&seed, "2026-06-13").is_none());
    }
}
