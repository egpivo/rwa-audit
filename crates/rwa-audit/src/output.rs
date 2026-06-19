use std::fs::File;
use std::io::Write;
use std::path::Path;

use chrono::Utc;

use crate::models::{
    ActivityDailyRow, HolderRow, MintBurnRow, QualityNote, RegistryRow, TransferRow,
};

pub fn write_registry(dir: &Path, rows: &[RegistryRow]) -> anyhow::Result<()> {
    let path = dir.join("rwa_asset_registry.csv");
    let mut wtr = csv::Writer::from_path(&path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    println!("  Wrote {}", path.display());
    Ok(())
}

pub fn write_transfer_metrics(dir: &Path, rows: &[TransferRow]) -> anyhow::Result<()> {
    let path = dir.join("rwa_transfer_metrics.csv");
    if rows.is_empty() {
        std::fs::write(
            &path,
            "asset_name,symbol,year_month,transfer_count,unique_senders,unique_receivers,total_volume_tokens,total_volume_usd_approx\n",
        )?;
    } else {
        let mut wtr = csv::Writer::from_path(&path)?;
        for r in rows {
            wtr.serialize(r)?;
        }
        wtr.flush()?;
    }
    println!("  Wrote {}", path.display());
    Ok(())
}

pub fn write_holder_metrics(dir: &Path, rows: &[HolderRow]) -> anyhow::Result<()> {
    let path = dir.join("rwa_holder_metrics.csv");
    let mut wtr = csv::Writer::from_path(&path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    println!("  Wrote {}", path.display());
    Ok(())
}

pub fn write_mint_burn_metrics(dir: &Path, rows: &[MintBurnRow]) -> anyhow::Result<()> {
    let path = dir.join("rwa_mint_burn_metrics.csv");
    if rows.is_empty() {
        std::fs::write(
            &path,
            "asset_name,symbol,year_month,mint_count,mint_volume_tokens,burn_count,burn_volume_tokens,net_issuance_tokens\n",
        )?;
    } else {
        let mut wtr = csv::Writer::from_path(&path)?;
        for r in rows {
            wtr.serialize(r)?;
        }
        wtr.flush()?;
    }
    println!("  Wrote {}", path.display());
    Ok(())
}

pub fn write_activity_daily(path: &Path, rows: &[ActivityDailyRow]) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    for r in rows {
        wtr.serialize(r)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn write_quality_notes(dir: &Path, notes: &[QualityNote]) -> anyhow::Result<()> {
    let path = dir.join("rwa_data_quality_notes.md");
    let mut f = File::create(&path)?;

    writeln!(f, "# RWA Data Quality Notes\n")?;
    writeln!(f, "Generated: {}\n", Utc::now().to_rfc3339())?;
    writeln!(f, "## Data Sources Used\n")?;
    writeln!(
        f,
        "- **On-chain logs**: publicnode.com public RPC endpoints (no API key required)"
    )?;
    writeln!(f, "  - Ethereum: https://ethereum.publicnode.com")?;
    writeln!(f, "  - Polygon: https://polygon.publicnode.com")?;
    writeln!(
        f,
        "- **Token info and holder data**: Ethplorer API (freekey, Ethereum only)"
    )?;
    writeln!(f, "- **Prices**: CoinGecko free API")?;
    writeln!(
        f,
        "- **Contract addresses**: Etherscan / official issuer documentation\n"
    )?;
    writeln!(f, "## General Caveats\n")?;
    writeln!(
        f,
        "- Transfer logs limited to last 6 months of on-chain history"
    )?;
    writeln!(
        f,
        "- publicnode RPC limits getLogs to 40,000 blocks per call; history fetched in chunks"
    )?;
    writeln!(f, "- USD approximations use CoinGecko spot prices at collection time; do not reflect official NAV")?;
    writeln!(
        f,
        "- Permissioning status is based on public documentation, not live contract inspection"
    )?;
    writeln!(
        f,
        "- Ethplorer holder data may lag on-chain state by hours; free tier rate-limited"
    )?;
    writeln!(
        f,
        "- BENJI (Franklin Templeton) Polygon data only; the canonical record is on Stellar"
    )?;
    writeln!(
        f,
        "- MPL and TRU are governance tokens, not direct RWA instruments; included as proxies\n"
    )?;
    writeln!(f, "## Per-Asset Notes\n")?;

    for note in notes {
        writeln!(f, "### {} ({}) — {}\n", note.name, note.symbol, note.chain)?;
        writeln!(f, "**Data Issues Encountered:**")?;
        for issue in &note.issues {
            writeln!(f, "- {issue}")?;
        }
        writeln!(f, "\n**Context:**")?;
        for ctx in &note.context {
            if !ctx.is_empty() {
                writeln!(f, "- {ctx}")?;
            }
        }
        writeln!(f)?;
    }

    write_structural_notes(&mut f)?;
    println!("  Wrote {}", path.display());
    Ok(())
}

fn write_structural_notes(f: &mut File) -> anyhow::Result<()> {
    writeln!(f, "## Asset-Specific Structural Notes\n")?;

    writeln!(f, "### Franklin Templeton BENJI (FOBXX)")?;
    writeln!(
        f,
        "- Primary blockchain record is on Stellar. The Polygon deployment is secondary."
    )?;
    writeln!(
        f,
        "- Polygon contract may show limited activity relative to Stellar."
    )?;
    writeln!(
        f,
        "- The fund is a SEC-registered money market mutual fund, not a DeFi protocol.\n"
    )?;

    writeln!(f, "### Ondo OUSG")?;
    writeln!(f, "- Restricted to US institutional investors with KYC/AML. Very low transfer count is expected.")?;
    writeln!(
        f,
        "- Official NAV updated daily; no continuous secondary market pricing.\n"
    )?;

    writeln!(f, "### Ondo USDY")?;
    writeln!(
        f,
        "- Accessible to non-US investors. More liquid than OUSG."
    )?;
    writeln!(
        f,
        "- Price appreciation model: NAV increases over time rather than rebasing.\n"
    )?;

    writeln!(f, "### BlackRock BUIDL")?;
    writeln!(
        f,
        "- Whitelist-controlled; only approved institutional investors can transact."
    )?;
    writeln!(f, "- Available on multiple chains (Ethereum, Arbitrum, Optimism, Avalanche). This study covers Ethereum only.")?;
    writeln!(
        f,
        "- As of early 2025, BUIDL was the largest tokenized treasury fund by AUM.\n"
    )?;

    writeln!(f, "### Superstate USTB")?;
    writeln!(
        f,
        "- Uses a separate PermissionList contract to gate transfers."
    )?;
    writeln!(f, "- NAV oracle updates continuously (second-by-second).\n")?;

    writeln!(f, "### Paxos Gold (PAXG)")?;
    writeln!(
        f,
        "- Most liquid tokenized gold on Ethereum with deep DEX activity."
    )?;
    writeln!(
        f,
        "- 1:1 backed by LBMA gold bars; Paxos is the custodian.\n"
    )?;

    writeln!(f, "### Tether Gold (XAUT)")?;
    writeln!(f, "- Issued by Tether; 1 XAUT = 1 troy oz on LBMA bars.")?;
    writeln!(f, "- Less DeFi integration than PAXG.\n")?;

    writeln!(f, "### Maple Finance MPL")?;
    writeln!(
        f,
        "- Governance token, not a direct loan/credit instrument."
    )?;
    writeln!(
        f,
        "- Protocol migration toward Syrup token (SYRUP) underway as of 2024.\n"
    )?;

    writeln!(f, "### TrueFi TRU")?;
    writeln!(
        f,
        "- Governance/staking token for the TrueFi lending protocol."
    )?;
    writeln!(
        f,
        "- TrueFi TVL has declined significantly since 2022 peak.\n"
    )?;

    writeln!(f, "### Backed bIB01 / bCSPX")?;
    writeln!(
        f,
        "- Issued under Swiss DLT law; secondary transfers unrestricted."
    )?;
    writeln!(
        f,
        "- Minting/redemption only through Backed platform with KYC."
    )?;
    writeln!(
        f,
        "- Low trading volumes; primarily held by institutional DeFi protocols.\n"
    )?;

    Ok(())
}

// Serde derives for csv output — attach to models via impl blocks in output module
use serde::Serialize;

impl Serialize for RegistryRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("RegistryRow", 11)?;
        s.serialize_field("asset_name", &self.asset_name)?;
        s.serialize_field("symbol", &self.symbol)?;
        s.serialize_field("category", &self.category)?;
        s.serialize_field("chain", &self.chain)?;
        s.serialize_field("contract_address", &self.contract_address)?;
        s.serialize_field("decimals", &self.decimals)?;
        s.serialize_field("total_supply", &self.total_supply)?;
        s.serialize_field("total_supply_usd_approx", &self.total_supply_usd_approx)?;
        s.serialize_field("is_permissioned", &self.is_permissioned)?;
        s.serialize_field("data_source", &self.data_source)?;
        s.serialize_field("notes", &self.notes)?;
        s.end()
    }
}

impl Serialize for TransferRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("TransferRow", 8)?;
        s.serialize_field("asset_name", &self.asset_name)?;
        s.serialize_field("symbol", &self.symbol)?;
        s.serialize_field("year_month", &self.year_month)?;
        s.serialize_field("transfer_count", &self.transfer_count)?;
        s.serialize_field("unique_senders", &self.unique_senders)?;
        s.serialize_field("unique_receivers", &self.unique_receivers)?;
        s.serialize_field("total_volume_tokens", &self.total_volume_tokens)?;
        s.serialize_field("total_volume_usd_approx", &self.total_volume_usd_approx)?;
        s.end()
    }
}

impl Serialize for HolderRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("HolderRow", 7)?;
        s.serialize_field("asset_name", &self.asset_name)?;
        s.serialize_field("symbol", &self.symbol)?;
        s.serialize_field("holder_count", &self.holder_count)?;
        s.serialize_field("top10_concentration_pct", &self.top10_concentration_pct)?;
        s.serialize_field("top1_concentration_pct", &self.top1_concentration_pct)?;
        s.serialize_field("data_as_of", &self.data_as_of)?;
        s.serialize_field("data_source", &self.data_source)?;
        s.end()
    }
}

impl Serialize for MintBurnRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("MintBurnRow", 8)?;
        s.serialize_field("asset_name", &self.asset_name)?;
        s.serialize_field("symbol", &self.symbol)?;
        s.serialize_field("year_month", &self.year_month)?;
        s.serialize_field("mint_count", &self.mint_count)?;
        s.serialize_field("mint_volume_tokens", &self.mint_volume_tokens)?;
        s.serialize_field("burn_count", &self.burn_count)?;
        s.serialize_field("burn_volume_tokens", &self.burn_volume_tokens)?;
        s.serialize_field("net_issuance_tokens", &self.net_issuance_tokens)?;
        s.end()
    }
}

impl Serialize for ActivityDailyRow {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("ActivityDailyRow", 12)?;
        s.serialize_field("date", &self.date)?;
        s.serialize_field("product_or_platform", &self.product_or_platform)?;
        s.serialize_field("workflow_type", &self.workflow_type)?;
        s.serialize_field("chain_or_venue", &self.chain_or_venue)?;
        s.serialize_field("volume_metric_type", &self.volume_metric_type)?;
        s.serialize_field("volume_usd", &self.volume_usd)?;
        s.serialize_field("volume_tokens", &self.volume_tokens)?;
        s.serialize_field("active_user_metric_type", &self.active_user_metric_type)?;
        s.serialize_field("active_user_count", &self.active_user_count)?;
        s.serialize_field("observation_domain", &self.observation_domain)?;
        s.serialize_field("source", &self.source)?;
        s.serialize_field("include_in_figure", &self.include_in_figure)?;
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_output_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rwa-audit-output-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn write_registry_and_transfer_metrics_roundtrip_headers() {
        let dir = temp_output_dir();
        fs::create_dir_all(&dir).unwrap();

        write_registry(
            &dir,
            &[RegistryRow {
                asset_name: "Test".into(),
                symbol: "TST".into(),
                category: "Test".into(),
                chain: "Ethereum".into(),
                contract_address: "0xabc".into(),
                decimals: 18,
                total_supply: "1.0".into(),
                total_supply_usd_approx: "1.0".into(),
                is_permissioned: "false".into(),
                data_source: "test".into(),
                notes: "".into(),
            }],
        )
        .unwrap();

        write_transfer_metrics(&dir, &[]).unwrap();
        write_mint_burn_metrics(&dir, &[]).unwrap();

        let registry = fs::read_to_string(dir.join("rwa_asset_registry.csv")).unwrap();
        assert!(registry.contains("asset_name,symbol"));
        assert!(registry.contains("TST"));

        let transfer = fs::read_to_string(dir.join("rwa_transfer_metrics.csv")).unwrap();
        assert!(transfer.starts_with("asset_name,symbol,year_month"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_activity_daily_and_quality_notes() {
        let dir = temp_output_dir();
        fs::create_dir_all(&dir).unwrap();
        let daily_path = dir.join("daily.csv");

        write_activity_daily(
            &daily_path,
            &[ActivityDailyRow {
                date: "2026-01-01".into(),
                product_or_platform: "BUIDL".into(),
                workflow_type: "test".into(),
                chain_or_venue: "Ethereum".into(),
                volume_metric_type: "test".into(),
                volume_usd: "100.0".into(),
                volume_tokens: "100.0".into(),
                active_user_metric_type: "senders".into(),
                active_user_count: 3,
                observation_domain: "test".into(),
                source: "test".into(),
                include_in_figure: "yes".into(),
            }],
        )
        .unwrap();

        write_quality_notes(
            &dir,
            &[QualityNote {
                name: "Test Asset".into(),
                symbol: "TST".into(),
                chain: "Ethereum".into(),
                issues: vec!["missing price".into()],
                context: vec!["fixture".into()],
            }],
        )
        .unwrap();

        let daily = fs::read_to_string(daily_path).unwrap();
        assert!(daily.contains("BUIDL"));
        let notes = fs::read_to_string(dir.join("rwa_data_quality_notes.md")).unwrap();
        assert!(notes.contains("missing price"));

        let _ = fs::remove_dir_all(dir);
    }
}
