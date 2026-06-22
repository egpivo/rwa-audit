//! Integration tests for CSV pipeline outputs.

use rwa_audit::models::{ActivityDailyRow, HolderRow, MintBurnRow, RegistryRow, TransferRow};
use rwa_audit::output::{
    write_activity_daily, write_holder_metrics, write_mint_burn_metrics, write_quality_notes,
    write_registry, write_transfer_metrics,
};

fn temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rwa-audit-it-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn full_csv_pipeline_writes_expected_files() {
    let dir = temp_dir();
    std::fs::create_dir_all(&dir).unwrap();

    write_registry(
        &dir,
        &[RegistryRow {
            asset_name: "BlackRock BUIDL".into(),
            symbol: "BUIDL".into(),
            category: "Tokenized Treasury/MMF".into(),
            chain: "Ethereum".into(),
            contract_address: "0x7712c34205737192402172409a8f7ccef8aa2aec".into(),
            decimals: 6,
            total_supply: "100.0".into(),
            total_supply_usd_approx: "100.0".into(),
            is_permissioned: "true".into(),
            data_source: "test".into(),
            notes: "fixture".into(),
        }],
    )
    .unwrap();

    write_transfer_metrics(
        &dir,
        &[TransferRow {
            asset_name: "BlackRock BUIDL".into(),
            symbol: "BUIDL".into(),
            year_month: "2026-05".into(),
            transfer_count: 42,
            unique_senders: 10,
            unique_receivers: 12,
            total_volume_tokens: 1_000.0,
            total_volume_usd_approx: "1000.0".into(),
        }],
    )
    .unwrap();

    write_holder_metrics(
        &dir,
        &[HolderRow {
            asset_name: "BlackRock BUIDL".into(),
            symbol: "BUIDL".into(),
            holder_count: "50".into(),
            top10_concentration_pct: "80.0".into(),
            top1_concentration_pct: "25.0".into(),
            data_as_of: "2026-06-01".into(),
            data_source: "test".into(),
        }],
    )
    .unwrap();

    write_mint_burn_metrics(
        &dir,
        &[MintBurnRow {
            asset_name: "BlackRock BUIDL".into(),
            symbol: "BUIDL".into(),
            year_month: "2026-05".into(),
            mint_count: 2,
            mint_volume_tokens: 500.0,
            burn_count: 1,
            burn_volume_tokens: 100.0,
            net_issuance_tokens: 400.0,
        }],
    )
    .unwrap();

    write_quality_notes(&dir, &[]).unwrap();

    let daily_path = dir.join("rwa_activity_daily_30d.csv");
    write_activity_daily(
        &daily_path,
        &[ActivityDailyRow {
            date: "2026-05-01".into(),
            product_or_platform: "BUIDL".into(),
            workflow_type: "Token-level ERC-20 observable activity".into(),
            chain_or_venue: "Ethereum".into(),
            volume_metric_type: "Daily ERC-20 transfer volume (USD approx)".into(),
            volume_usd: "10.0".into(),
            volume_tokens: "10.0".into(),
            active_user_metric_type: "Daily unique senders".into(),
            active_user_count: 4,
            observation_domain: "fixture".into(),
            source: "fixture".into(),
            include_in_figure: "yes".into(),
        }],
    )
    .unwrap();

    for name in [
        "rwa_asset_registry.csv",
        "rwa_transfer_metrics.csv",
        "rwa_holder_metrics.csv",
        "rwa_mint_burn_metrics.csv",
        "rwa_data_quality_notes.md",
        "rwa_activity_daily_30d.csv",
    ] {
        let path = if name.ends_with("30d.csv") {
            daily_path.clone()
        } else {
            dir.join(name)
        };
        assert!(path.exists(), "missing {}", path.display());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty(), "empty {}", path.display());
    }

    let transfer = std::fs::read_to_string(dir.join("rwa_transfer_metrics.csv")).unwrap();
    assert!(transfer.contains("2026-05"));
    assert!(transfer.contains("BUIDL"));

    let _ = std::fs::remove_dir_all(dir);
}
