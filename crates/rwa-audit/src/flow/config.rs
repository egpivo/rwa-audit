use chrono::NaiveDate;

pub const GECKO_BASE: &str = "https://api.geckoterminal.com/api/v2";
pub const PARASWAP_BASE: &str = "https://apiv5.paraswap.io";
pub const YAHOO_GC_CHART: &str = "https://query1.finance.yahoo.com/v8/finance/chart/GC=F";

pub const PANEL_START: &str = "2026-03-10";
pub const PANEL_END: &str = "2026-06-08";
pub const PANEL_NETWORK: &str = "eth";

pub const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

pub const GECKO_SLEEP_MS: u64 = 3_500;
pub const MAX_POOLS_PER_TOKEN: usize = 25;
pub const PARASWAP_SLEEP_MS: u64 = 1_000;

pub const MIN_POOL_VOLUME_USD: f64 = 100.0;
/// Solana pool-search aggregates exclude pools above this TVL (mis-tagged outliers).
pub const OUTLIER_TVL_USD: f64 = 50_000_000.0;

pub struct PanelToken {
    pub symbol: &'static str,
    pub address: &'static str,
    pub decimals: u32,
    pub approx_price_usd: f64,
}

pub const PANEL_TOKENS: &[PanelToken] = &[
    PanelToken {
        symbol: "PAXG",
        address: "0x45804880de22913dafe09f4980848ece6ecbaf78",
        decimals: 18,
        approx_price_usd: 4_200.0,
    },
    PanelToken {
        symbol: "USDY",
        address: "0x96f6ef951840721adbf46ac996b59e0235cb985c",
        decimals: 18,
        approx_price_usd: 1.0,
    },
];

pub const QUOTE_USD_SIZES: &[u64] = &[1_000, 10_000, 100_000];

pub const JUPITER_QUOTE_BASE: &str = "https://lite-api.jup.ag/swap/v1/quote";
pub const USDC_SOLANA: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const AAPLX_SOLANA: &str = "XsbEhLAtcf6HdfpFZ5xEMdqW8nfAvcsP5bdudRLJzJp";
pub const JUPITER_QUOTE_USD: u64 = 100_000;
pub const JUPITER_SLIPPAGE_BPS: u32 = 100;

pub fn panel_start_date() -> NaiveDate {
    NaiveDate::parse_from_str(PANEL_START, "%Y-%m-%d").expect("panel start")
}

pub fn panel_end_date() -> NaiveDate {
    NaiveDate::parse_from_str(PANEL_END, "%Y-%m-%d").expect("panel end")
}

pub fn weekly_checkpoints() -> Vec<NaiveDate> {
    let start = panel_start_date();
    let end = panel_end_date();
    let mut dates = Vec::new();
    let mut cur = start;
    while cur <= end {
        dates.push(cur);
        cur += chrono::Duration::days(7);
    }
    if dates.last() != Some(&end) {
        dates.push(end);
    }
    dates
}

pub fn flow_data_dir() -> std::path::PathBuf {
    crate::config::data_dir().join("flow")
}
