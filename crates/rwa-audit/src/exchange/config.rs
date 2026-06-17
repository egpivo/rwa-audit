use std::path::PathBuf;

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

pub fn exchange_data_dir() -> PathBuf {
    crate::config::artifacts_data_dir()
}

pub fn platform_seed_path() -> PathBuf {
    exchange_data_dir().join("rwa_xyz_platform_transfer_snapshots.json")
}

pub fn bridged_export_csv() -> PathBuf {
    exchange_data_dir().join("rwa-token-timeseries-export-1781314094816.csv")
}

pub fn reference_panel_path() -> PathBuf {
    exchange_data_dir().join("depth_vs_volume_panel.csv")
}

pub fn publish_panel_path() -> PathBuf {
    exchange_data_dir().join("depth_vs_volume_panel_publish.csv")
}

pub fn jupiter_publish_fixture_path() -> PathBuf {
    exchange_data_dir().join("jupiter_quote_aaplx_100k_publish.json")
}

pub fn manifest_path() -> PathBuf {
    exchange_data_dir().join("manifest.json")
}
