use std::path::{Path, PathBuf};

pub const RWA_XSTOCKS_URL: &str = "https://app.rwa.xyz/platforms/xstocks";

pub const TSLAX_SOLANA: &str = "XsDoVfqeBukxuZHWhdvDnFcN8EzRz9jGfY9Smo9SxvH";
pub const SPYX_SOLANA: &str = "XsoCS1TfEyfFhfvj8EtZ528L3CaKBDBRqRapnBbDF2W";

pub const PUBLISH_PANEL_DATE: &str = "2026-06-12";
pub const BRIDGED_VALUE_DATE: &str = "2026-06-11";
pub const PLATFORM_TRANSFER_APR_DATE: &str = "2026-04-20";
pub const PLATFORM_TRANSFER_JUN_DATE: &str = "2026-06-12";

pub struct XStockMint {
    pub symbol: &'static str,
    pub mint: &'static str,
}

pub const XSTOCKS_SOLANA: &[XStockMint] = &[
    XStockMint {
        symbol: "AAPLx",
        mint: crate::flow::config::AAPLX_SOLANA,
    },
    XStockMint {
        symbol: "TSLAx",
        mint: TSLAX_SOLANA,
    },
    XStockMint {
        symbol: "SPYx",
        mint: SPYX_SOLANA,
    },
];

/// Committed publish staging (`artifacts/data/`).
pub fn exchange_publish_dir() -> PathBuf {
    crate::config::artifacts_data_dir()
}

/// Live API refresh scratch (`data/exchange-live/`); never promoted directly.
pub fn exchange_live_staging_dir() -> PathBuf {
    crate::config::data_dir().join("exchange-live")
}

pub fn exchange_output_dir(live_apis: bool) -> PathBuf {
    if live_apis {
        exchange_live_staging_dir()
    } else {
        exchange_publish_dir()
    }
}

/// Fixture paths (read-only inputs) always live under publish dir.
pub fn exchange_data_dir() -> PathBuf {
    exchange_publish_dir()
}

pub fn platform_seed_path() -> PathBuf {
    exchange_publish_dir().join("rwa_xyz_platform_transfer_snapshots.json")
}

pub fn bridged_export_csv() -> PathBuf {
    exchange_publish_dir().join("rwa-token-timeseries-export-1781314094816.csv")
}

pub fn reference_panel_path() -> PathBuf {
    exchange_publish_dir().join("depth_vs_volume_panel.csv")
}

pub fn publish_panel_path() -> PathBuf {
    exchange_publish_dir().join("depth_vs_volume_panel_publish.csv")
}

pub fn jupiter_publish_fixture_path() -> PathBuf {
    exchange_publish_dir().join("jupiter_quote_aaplx_100k_publish.json")
}

pub fn manifest_path() -> PathBuf {
    exchange_publish_dir().join("manifest.json")
}

/// Live refresh scratch audit id (not a publish bundle).
pub fn exchange_live_audit_id(panel_date: &str) -> String {
    format!("exchange-live-{panel_date}")
}

pub fn evidence_path_in_dir(out_dir: &Path, filename: &str) -> String {
    crate::config::path_to_repo_relative(&out_dir.join(filename))
}

/// Offline freeze uses fixed publish fixtures; custom dates would mislabel evidence.
pub fn resolve_panel_date(live_apis: bool, panel_date: Option<&str>) -> anyhow::Result<String> {
    if live_apis {
        return Ok(panel_date
            .map(str::to_string)
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string()));
    }
    match panel_date {
        None | Some(PUBLISH_PANEL_DATE) => Ok(PUBLISH_PANEL_DATE.into()),
        Some(other) => anyhow::bail!(
            "offline freeze only supports publish panel date {PUBLISH_PANEL_DATE} (got {other}); \
             use --mode live for fresh API evidence or omit --publish-date"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_panel_date_must_match_publish_fixture() {
        assert_eq!(resolve_panel_date(false, None).unwrap(), PUBLISH_PANEL_DATE);
        assert_eq!(
            resolve_panel_date(false, Some(PUBLISH_PANEL_DATE)).unwrap(),
            PUBLISH_PANEL_DATE
        );
        assert!(resolve_panel_date(false, Some("2026-06-15")).is_err());
    }

    #[test]
    fn live_panel_date_accepts_custom_label() {
        assert_eq!(
            resolve_panel_date(true, Some("2026-06-20")).unwrap(),
            "2026-06-20"
        );
    }

    #[test]
    fn live_output_uses_staging_dir() {
        let staging = exchange_output_dir(true);
        assert!(staging.ends_with("data/exchange-live"));
        let publish = exchange_output_dir(false);
        assert!(publish.ends_with("artifacts/data"));
    }
}
