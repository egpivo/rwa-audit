use crate::models::{ActivityAsset, RegistryAsset};

pub fn registry_assets() -> Vec<RegistryAsset> {
    vec![
        RegistryAsset {
            asset_name: "Franklin Templeton BENJI (FOBXX)".into(),
            symbol: "BENJI".into(),
            category: "Tokenized Treasury/MMF".into(),
            chain: "Polygon".into(),
            contract_address: "0x408a634b8a8f0de729b48574a3a7ec3fe820b00a".into(),
            decimals: 6,
            coingecko_id: None,
            price_usd_approx: Some(1.0),
            notes: "Franklin OnChain U.S. Government Money Fund; Polygon deployment; originally on Stellar".into(),
        },
        RegistryAsset {
            asset_name: "Ondo OUSG".into(),
            symbol: "OUSG".into(),
            category: "Tokenized Treasury/MMF".into(),
            chain: "Ethereum".into(),
            contract_address: "0x1b19c19393e2d034d8ff31ff34c81252fcbbee92".into(),
            decimals: 18,
            coingecko_id: None,
            price_usd_approx: None,
            notes: "Ondo Short-Term U.S. Government Bond Fund; permissioned ERC-20; KYC required".into(),
        },
        RegistryAsset {
            asset_name: "Ondo USDY".into(),
            symbol: "USDY".into(),
            category: "Tokenized Treasury/MMF".into(),
            chain: "Ethereum".into(),
            contract_address: "0x96F6eF951840721AdBF46Ac996b59E0235CB985C".into(),
            decimals: 18,
            coingecko_id: Some("ondo-us-dollar-yield".into()),
            price_usd_approx: None,
            notes: "Ondo U.S. Dollar Yield; non-rebasing; accessible to non-US investors".into(),
        },
        RegistryAsset {
            asset_name: "BlackRock BUIDL".into(),
            symbol: "BUIDL".into(),
            category: "Tokenized Treasury/MMF".into(),
            chain: "Ethereum".into(),
            contract_address: "0x7712c34205737192402172409a8F7ccef8aA2AEc".into(),
            decimals: 6,
            coingecko_id: Some("blackrock-usd-institutional-digital-liquidity-fund".into()),
            price_usd_approx: Some(1.0),
            notes: "BlackRock USD Institutional Digital Liquidity Fund; permissioned".into(),
        },
        RegistryAsset {
            asset_name: "Superstate USTB".into(),
            symbol: "USTB".into(),
            category: "Tokenized Treasury/MMF".into(),
            chain: "Ethereum".into(),
            contract_address: "0x43415eB6ff9DB7E26A15b704e7A3eDCe97d31C4e".into(),
            decimals: 6,
            coingecko_id: None,
            price_usd_approx: None,
            notes: "Superstate Short Duration US Government Securities Fund; permissioned".into(),
        },
        RegistryAsset {
            asset_name: "Paxos Gold".into(),
            symbol: "PAXG".into(),
            category: "Gold/Commodities".into(),
            chain: "Ethereum".into(),
            contract_address: "0x45804880De22913dAFE09f4980848ECE6EcbAf78".into(),
            decimals: 18,
            coingecko_id: Some("pax-gold".into()),
            price_usd_approx: None,
            notes: "1 PAXG = 1 troy oz gold; stored in Brinks vaults".into(),
        },
        RegistryAsset {
            asset_name: "Tether Gold".into(),
            symbol: "XAUT".into(),
            category: "Gold/Commodities".into(),
            chain: "Ethereum".into(),
            contract_address: "0x68749665ff8d2d112fa859aa293f07a622782f38".into(),
            decimals: 6,
            coingecko_id: Some("tether-gold".into()),
            price_usd_approx: None,
            notes: "Each XAUT represents 1 troy oz of gold on LBMA good delivery bars".into(),
        },
        RegistryAsset {
            asset_name: "Maple Finance MPL".into(),
            symbol: "MPL".into(),
            category: "Private Credit".into(),
            chain: "Ethereum".into(),
            contract_address: "0x33349B282065b0284d756F0577FB39c158F935e6".into(),
            decimals: 18,
            coingecko_id: Some("maple".into()),
            price_usd_approx: None,
            notes: "Maple Finance governance token; protocol facilitates institutional uncollateralized lending".into(),
        },
        RegistryAsset {
            asset_name: "TrueFi TRU".into(),
            symbol: "TRU".into(),
            category: "Private Credit".into(),
            chain: "Ethereum".into(),
            contract_address: "0x4c19596f5aaff459fa38b0f7ed92f11ae6543784".into(),
            decimals: 8,
            coingecko_id: Some("truefi".into()),
            price_usd_approx: None,
            notes: "TrueFi governance/staking token; protocol for uncollateralized on-chain lending".into(),
        },
        RegistryAsset {
            asset_name: "Backed bIB01".into(),
            symbol: "bIB01".into(),
            category: "Tokenized Fund/ETF".into(),
            chain: "Ethereum".into(),
            contract_address: "0xca30c93b02514f86d5c86a6e375e3a330b435fb5".into(),
            decimals: 18,
            coingecko_id: None,
            price_usd_approx: None,
            notes: "Backed IB01 Treasury Bond 0-1yr; tracks iShares Treasury Bond 0-1yr ETF".into(),
        },
        RegistryAsset {
            asset_name: "Backed bCSPX".into(),
            symbol: "bCSPX".into(),
            category: "Tokenized Fund/ETF".into(),
            chain: "Ethereum".into(),
            contract_address: "0x1e2c4fb7ede391d116e6b41cd0608260e8801d59".into(),
            decimals: 18,
            coingecko_id: None,
            price_usd_approx: None,
            notes: "Backed CSPX Core S&P 500; tracks iShares Core S&P 500 UCITS ETF".into(),
        },
    ]
}

pub fn activity_assets() -> Vec<ActivityAsset> {
    vec![
        ActivityAsset {
            symbol: "BUIDL".into(),
            asset_name: "BlackRock BUIDL".into(),
            chain: "Ethereum".into(),
            contract: "0x7712c34205737192402172409a8f7ccef8aa2aec".into(),
            decimals: 6,
            price_usd_approx: Some(1.0),
            include_in_figure: true,
        },
        ActivityAsset {
            symbol: "USDY".into(),
            asset_name: "Ondo USDY".into(),
            chain: "Ethereum".into(),
            contract: "0x96f6ef951840721adbf46ac996b59e0235cb985c".into(),
            decimals: 18,
            price_usd_approx: None,
            include_in_figure: true,
        },
        ActivityAsset {
            symbol: "PAXG".into(),
            asset_name: "Paxos Gold".into(),
            chain: "Ethereum".into(),
            contract: "0x45804880de22913dafe09f4980848ece6ecbaf78".into(),
            decimals: 18,
            price_usd_approx: None,
            include_in_figure: true,
        },
        ActivityAsset {
            symbol: "XAUT".into(),
            asset_name: "Tether Gold".into(),
            chain: "Ethereum".into(),
            contract: "0x68749665ff8d2d112fa859aa293f07a622782f38".into(),
            decimals: 6,
            price_usd_approx: None,
            include_in_figure: true,
        },
        ActivityAsset {
            symbol: "BENJI".into(),
            asset_name: "Franklin Templeton BENJI (FOBXX)".into(),
            chain: "Polygon".into(),
            contract: "0x408a634b8a8f0de729b48574a3a7ec3fe820b00a".into(),
            decimals: 18,
            price_usd_approx: Some(1.0),
            include_in_figure: false,
        },
    ]
}

pub fn detect_permissioning_from_known(symbol: &str) -> Option<String> {
    const PERMISSIONED: &[&str] = &["OUSG", "BUIDL", "USTB", "BENJI"];
    const NOT_PERMISSIONED: &[&str] = &["PAXG", "XAUT", "bIB01", "bCSPX", "MPL", "TRU"];
    const PARTIAL: &[&str] = &["USDY"];

    if PERMISSIONED.contains(&symbol) {
        Some("true".into())
    } else if NOT_PERMISSIONED.contains(&symbol) {
        Some("false".into())
    } else if PARTIAL.contains(&symbol) {
        Some("partial".into())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn detect_permissioning_from_known_symbols() {
        assert_eq!(detect_permissioning_from_known("BUIDL").as_deref(), Some("true"));
        assert_eq!(detect_permissioning_from_known("PAXG").as_deref(), Some("false"));
        assert_eq!(detect_permissioning_from_known("USDY").as_deref(), Some("partial"));
        assert!(detect_permissioning_from_known("UNKNOWN").is_none());
    }

    #[test]
    fn registry_assets_have_unique_symbols_and_addresses() {
        let assets = registry_assets();
        assert_eq!(assets.len(), 11);
        let symbols: HashSet<_> = assets.iter().map(|a| a.symbol.as_str()).collect();
        assert_eq!(symbols.len(), assets.len());
        for a in &assets {
            assert!(a.contract_address.starts_with("0x"));
            assert!(a.decimals <= 18);
        }
    }

    #[test]
    fn activity_assets_include_chart_subset() {
        let assets = activity_assets();
        assert_eq!(assets.len(), 5);
        assert!(assets.iter().any(|a| a.symbol == "BENJI" && !a.include_in_figure));
        assert!(assets.iter().filter(|a| a.include_in_figure).count() >= 4);
    }
}
