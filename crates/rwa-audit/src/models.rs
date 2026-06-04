use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct RegistryAsset {
    pub asset_name: String,
    pub symbol: String,
    pub category: String,
    pub chain: String,
    pub contract_address: String,
    pub decimals: u32,
    pub coingecko_id: Option<String>,
    pub price_usd_approx: Option<f64>,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct ActivityAsset {
    pub symbol: String,
    pub asset_name: String,
    pub chain: String,
    pub contract: String,
    pub decimals: u32,
    pub price_usd_approx: Option<f64>,
    pub include_in_figure: bool,
}

#[derive(Debug, Deserialize)]
pub struct RpcResponse {
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TransferLog {
    pub from: String,
    pub to: String,
    pub value: u128,
    pub year_month: String,
    pub block_number: u64,
}

#[derive(Debug, Default)]
pub struct MonthlyMetrics {
    pub transfer_count: u64,
    pub unique_senders: std::collections::HashSet<String>,
    pub unique_receivers: std::collections::HashSet<String>,
    pub total_volume_tokens: f64,
    pub mint_count: u64,
    pub mint_volume: f64,
    pub burn_count: u64,
    pub burn_volume: f64,
}

#[derive(Debug)]
pub struct RegistryRow {
    pub asset_name: String,
    pub symbol: String,
    pub category: String,
    pub chain: String,
    pub contract_address: String,
    pub decimals: u32,
    pub total_supply: String,
    pub total_supply_usd_approx: String,
    pub is_permissioned: String,
    pub data_source: String,
    pub notes: String,
}

#[derive(Debug)]
pub struct TransferRow {
    pub asset_name: String,
    pub symbol: String,
    pub year_month: String,
    pub transfer_count: u64,
    pub unique_senders: usize,
    pub unique_receivers: usize,
    pub total_volume_tokens: f64,
    pub total_volume_usd_approx: String,
}

#[derive(Debug)]
pub struct HolderRow {
    pub asset_name: String,
    pub symbol: String,
    pub holder_count: String,
    pub top10_concentration_pct: String,
    pub top1_concentration_pct: String,
    pub data_as_of: String,
    pub data_source: String,
}

#[derive(Debug)]
pub struct MintBurnRow {
    pub asset_name: String,
    pub symbol: String,
    pub year_month: String,
    pub mint_count: u64,
    pub mint_volume_tokens: f64,
    pub burn_count: u64,
    pub burn_volume_tokens: f64,
    pub net_issuance_tokens: f64,
}

#[derive(Debug)]
pub struct ActivityDailyRow {
    pub date: String,
    pub product_or_platform: String,
    pub workflow_type: String,
    pub chain_or_venue: String,
    pub volume_metric_type: String,
    pub volume_usd: String,
    pub volume_tokens: String,
    pub active_user_metric_type: String,
    pub active_user_count: u64,
    pub observation_domain: String,
    pub source: String,
    pub include_in_figure: String,
}

#[derive(Debug)]
pub struct QualityNote {
    pub name: String,
    pub symbol: String,
    pub chain: String,
    pub issues: Vec<String>,
    pub context: Vec<String>,
}
