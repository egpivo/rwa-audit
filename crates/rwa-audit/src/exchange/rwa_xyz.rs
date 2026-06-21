use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::exchange::config::platform_seed_path;
use crate::sources::fetch::{http_get_text_cached, response_text};
use crate::sources::{SourceContext, SourceId};

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

/// xStocks platform page URL from `config/sources.yaml` (`rwa_xyz.base_url`).
pub fn rwa_xstocks_url(ctx: &SourceContext) -> Result<String> {
    const DEFAULT_PATH: &str = "/platforms/xstocks";
    let base = ctx.http_base_url(SourceId::RwaXyz)?;
    let base = base.trim_end_matches('/');
    if base.contains("/platforms/") {
        return Ok(base.to_string());
    }
    Ok(format!("{base}{DEFAULT_PATH}"))
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

    let ctx = SourceContext::for_force_refresh()?;
    let fetch_url = rwa_xstocks_url(&ctx)?;
    let resp = http_get_text_cached(SourceId::RwaXyz, &ctx, &fetch_url, &[])?;
    let source_url = resp.provenance.request_url.clone();
    let html = response_text(&resp)?;

    let json_str = extract_next_data(&html)?;
    let page: Value = serde_json::from_str(&json_str)?;
    let platform = &page["props"]["pageProps"]["platform"];
    let trailing = platform
        .get("trailing_30_day_transfer_volume")
        .cloned()
        .unwrap_or(Value::Null);

    let mut scraped = lag_snapshots(access_date, &trailing, &source_url);
    if let Some(headline) = headline_snapshot(
        access_date,
        &page,
        trailing.get("val_7d").and_then(|v| v.as_f64()),
        &source_url,
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

fn lag_snapshots(access_date: &str, trailing: &Value, source_url: &str) -> Vec<PlatformSnapshot> {
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
            source_url: source_url.into(),
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
    source_url: &str,
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
                source_url: source_url.into(),
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
        source_url: source_url.into(),
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

    #[test]
    fn extract_next_data_parses_valid_html() {
        let html = r#"<html><head><script id="__NEXT_DATA__" type="application/json">{"foo":"bar"}</script></head></html>"#;
        let result = extract_next_data(html).unwrap();
        assert_eq!(result, r#"{"foo":"bar"}"#);
    }

    #[test]
    fn extract_next_data_fails_without_marker() {
        let html = "<html><body>no next data here</body></html>";
        assert!(extract_next_data(html).is_err());
    }

    #[test]
    fn lag_snapshots_produces_entries_for_positive_values() {
        let trailing = serde_json::json!({
            "val_7d": 1_500_000_000.0,
            "val_30d": 1_400_000_000.0,
            "val_90d": 0.0
        });
        let snaps = lag_snapshots("2026-06-12", &trailing, "https://example.com");
        // val_7d and val_30d are positive; val_90d = 0 is skipped
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].confidence, "high");
    }

    #[test]
    fn lag_snapshots_skips_zero_values() {
        let trailing = serde_json::json!({
            "val_7d": 0.0,
            "val_30d": 0.0,
            "val_90d": 0.0
        });
        let snaps = lag_snapshots("2026-06-12", &trailing, "https://example.com");
        assert!(snaps.is_empty());
    }

    #[test]
    fn lag_snapshots_invalid_date_returns_empty() {
        let trailing = serde_json::json!({ "val_7d": 1_000_000.0 });
        let snaps = lag_snapshots("not-a-date", &trailing, "https://example.com");
        assert!(snaps.is_empty());
    }

    #[test]
    fn headline_snapshot_suspect_when_val_too_low_vs_trailing() {
        // val = 1M, trailing_7d = 2B → ratio = 0.0005 < 0.05 → suspect
        let page = serde_json::json!({
            "props": {
                "pageProps": {
                    "aggregates": [
                        {"label": "Monthly Transfer Volume", "value": 1_000_000.0}
                    ]
                }
            }
        });
        let snap = headline_snapshot(
            "2026-06-12",
            &page,
            Some(2_000_000_000.0),
            "https://example.com",
        )
        .unwrap();
        assert_eq!(snap.confidence, "low");
        assert_eq!(
            snap.source_type.as_deref(),
            Some("rwa_ssr_headline_suspect")
        );
    }

    #[test]
    fn headline_snapshot_normal_case() {
        let page = serde_json::json!({
            "props": {
                "pageProps": {
                    "aggregates": [
                        {"label": "Monthly Transfer Volume", "value": 1_600_000_000.0}
                    ]
                }
            }
        });
        let snap = headline_snapshot(
            "2026-06-12",
            &page,
            Some(1_500_000_000.0),
            "https://example.com",
        )
        .unwrap();
        assert_eq!(snap.confidence, "high");
        assert_eq!(snap.source_type.as_deref(), Some("rwa_ssr_headline"));
    }

    #[test]
    fn headline_snapshot_returns_none_without_aggregates() {
        let page = serde_json::json!({
            "props": {
                "pageProps": {}
            }
        });
        assert!(headline_snapshot("2026-06-12", &page, None, "https://example.com").is_none());
    }

    #[test]
    fn headline_snapshot_returns_none_without_matching_label() {
        let page = serde_json::json!({
            "props": {
                "pageProps": {
                    "aggregates": [
                        {"label": "Wrong Label", "value": 1_000_000.0}
                    ]
                }
            }
        });
        assert!(headline_snapshot("2026-06-12", &page, None, "https://example.com").is_none());
    }

    fn make_snap(date: &str, confidence: &str) -> PlatformSnapshot {
        PlatformSnapshot {
            date: date.into(),
            monthly_transfer_volume_usd: 1.0e9,
            source_url: "u".into(),
            accessed_date: date.into(),
            confidence: confidence.into(),
            caveat: "c".into(),
            source_type: None,
            exclude_from_interpolation: None,
        }
    }

    #[test]
    fn merge_snapshots_higher_confidence_wins() {
        let existing = vec![make_snap("2026-06-12", "low")];
        let new = vec![make_snap("2026-06-12", "high")];
        let result = merge_snapshots(&existing, &new);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].confidence, "high");
    }

    #[test]
    fn merge_snapshots_same_confidence_keeps_existing() {
        let existing = vec![make_snap("2026-06-12", "high")];
        let new = vec![make_snap("2026-06-12", "high")];
        // existing is inserted first; new has same rank → existing kept
        let result = merge_snapshots(&existing, &new);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn merge_snapshots_adds_new_date() {
        let existing = vec![make_snap("2026-06-11", "high")];
        let new = vec![make_snap("2026-06-12", "high")];
        let result = merge_snapshots(&existing, &new);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn save_and_load_seed_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-xyz-seed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("seed.json");
        let seed = PlatformSeedFile {
            metric: "monthly_transfer_volume".into(),
            definition_url: "https://docs.rwa.xyz".into(),
            note: "test".into(),
            last_fetched: Some("2026-06-12".into()),
            snapshots: vec![make_snap("2026-06-12", "high")],
        };
        save_seed_to(&path, &seed).unwrap();
        let loaded = load_seed_from(&path).unwrap();
        assert_eq!(loaded.metric, "monthly_transfer_volume");
        assert_eq!(loaded.snapshots.len(), 1);
        assert_eq!(loaded.snapshots[0].date, "2026-06-12");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn load_seed_from_missing_file_errors() {
        let path = std::path::Path::new("/tmp/does-not-exist-rwa-xyz-seed-12345.json");
        assert!(load_seed_from(path).is_err());
    }

    #[test]
    fn snapshot_for_date_returns_low_confidence_non_suspect() {
        // low confidence but NOT rwa_ssr_headline_suspect → should be included
        let seed = PlatformSeedFile {
            metric: "x".into(),
            definition_url: "x".into(),
            note: "x".into(),
            last_fetched: None,
            snapshots: vec![PlatformSnapshot {
                date: "2026-06-10".into(),
                monthly_transfer_volume_usd: 1.0e6,
                source_url: "u".into(),
                accessed_date: "2026-06-10".into(),
                confidence: "low".into(),
                caveat: "c".into(),
                source_type: Some("rwa_ssr_trailing_30d_lag".into()),
                exclude_from_interpolation: None,
            }],
        };
        // low + non-suspect source_type → not filtered out
        assert!(snapshot_for_date(&seed, "2026-06-10").is_some());
    }

    #[test]
    fn snapshot_for_date_missing_date_returns_none() {
        let seed = PlatformSeedFile {
            metric: "x".into(),
            definition_url: "x".into(),
            note: "x".into(),
            last_fetched: None,
            snapshots: vec![PlatformSnapshot {
                date: "2026-06-10".into(),
                monthly_transfer_volume_usd: 1.0e6,
                source_url: "u".into(),
                accessed_date: "2026-06-10".into(),
                confidence: "high".into(),
                caveat: "".into(),
                source_type: None,
                exclude_from_interpolation: None,
            }],
        };
        assert!(snapshot_for_date(&seed, "2026-06-11").is_none());
    }

    #[test]
    fn save_seed_to_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-seed-rt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("seed.json");
        let seed = PlatformSeedFile {
            metric: "test".into(),
            definition_url: "https://example".into(),
            note: "note".into(),
            last_fetched: Some("2026-06-12".into()),
            snapshots: vec![],
        };
        save_seed_to(&path, &seed).unwrap();
        let loaded = load_seed_from(&path).unwrap();
        assert_eq!(loaded.metric, "test");
        assert_eq!(loaded.last_fetched, Some("2026-06-12".into()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rwa_xstocks_url_appends_path_when_base_has_no_platform() {
        use crate::sources::cache::ResponseCache;
        use crate::sources::profile::{SourceKind, SourceProfile};
        use crate::sources::registry::SourceRegistry;
        use std::collections::HashMap;
        let profile = SourceProfile {
            id: SourceId::RwaXyz,
            kind: SourceKind::Http,
            base_url: Some("https://app.rwa.xyz".into()),
            rpc_endpoints: HashMap::new(),
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::new(),
        };
        let reg = SourceRegistry::from_profiles(HashMap::from([(SourceId::RwaXyz, profile)]));
        let ctx = crate::sources::context::SourceContext::with_registry(reg)
            .unwrap()
            .with_cache(ResponseCache::disabled());
        let url = rwa_xstocks_url(&ctx).unwrap();
        assert_eq!(url, "https://app.rwa.xyz/platforms/xstocks");
    }

    #[test]
    fn rwa_xstocks_url_returns_base_when_already_contains_platforms_path() {
        use crate::sources::cache::ResponseCache;
        use crate::sources::profile::{SourceKind, SourceProfile};
        use crate::sources::registry::SourceRegistry;
        use std::collections::HashMap;
        let profile = SourceProfile {
            id: SourceId::RwaXyz,
            kind: SourceKind::Http,
            base_url: Some("https://app.rwa.xyz/platforms/xstocks".into()),
            rpc_endpoints: HashMap::new(),
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::new(),
        };
        let reg = SourceRegistry::from_profiles(HashMap::from([(SourceId::RwaXyz, profile)]));
        let ctx = crate::sources::context::SourceContext::with_registry(reg)
            .unwrap()
            .with_cache(ResponseCache::disabled());
        let url = rwa_xstocks_url(&ctx).unwrap();
        assert_eq!(url, "https://app.rwa.xyz/platforms/xstocks");
    }
}
