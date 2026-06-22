use crate::assets::detect_permissioning_from_known;
use crate::tools::types::ToolResult;
use crate::tools::TOOL_CLASSIFY_SURFACE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceType {
    OpenTransferSurface,
    PermissionedCounterpartyRail,
    RoutedOperationalActivity,
    IncompleteContractVisibility,
}

impl SurfaceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenTransferSurface => "open_transfer_surface",
            Self::PermissionedCounterpartyRail => "permissioned_counterparty_rail",
            Self::RoutedOperationalActivity => "routed_operational_activity",
            Self::IncompleteContractVisibility => "incomplete_contract_visibility",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssetSurfaceInput {
    pub symbol: String,
    pub holder_count: Option<u64>,
    pub top10_concentration_pct: Option<f64>,
    pub monthly_unique_senders: Option<u64>,
    pub monthly_transfer_count: Option<u64>,
    pub data_incomplete: bool,
}

/// Article 1: classify visible ERC-20 activity into surface types.
pub fn classify_surface_type(assets: &[AssetSurfaceInput]) -> ToolResult {
    let mut result = ToolResult::new(TOOL_CLASSIFY_SURFACE);
    for asset in assets {
        let surface = classify_one(asset);
        result = result
            .label(format!("{}: {}", asset.symbol, surface.as_str()))
            .metric(
                format!("{}_surface_type_code", asset.symbol),
                surface_type_code(surface),
                "enum",
            );
    }
    result
}

fn classify_one(asset: &AssetSurfaceInput) -> SurfaceType {
    if asset.data_incomplete {
        return SurfaceType::IncompleteContractVisibility;
    }

    let permissioning = detect_permissioning_from_known(&asset.symbol);
    match permissioning.as_deref() {
        Some("true") => SurfaceType::PermissionedCounterpartyRail,
        Some("partial") => SurfaceType::RoutedOperationalActivity,
        Some("false") => SurfaceType::OpenTransferSurface,
        _ => {
            if asset.holder_count.is_none() {
                SurfaceType::IncompleteContractVisibility
            } else {
                SurfaceType::RoutedOperationalActivity
            }
        }
    }
}

fn surface_type_code(surface: SurfaceType) -> f64 {
    match surface {
        SurfaceType::OpenTransferSurface => 1.0,
        SurfaceType::PermissionedCounterpartyRail => 2.0,
        SurfaceType::RoutedOperationalActivity => 3.0,
        SurfaceType::IncompleteContractVisibility => 4.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_buidl_as_permissioned_rail() {
        let assets = vec![AssetSurfaceInput {
            symbol: "BUIDL".into(),
            holder_count: Some(59),
            top10_concentration_pct: Some(82.0),
            monthly_unique_senders: Some(10),
            monthly_transfer_count: Some(100),
            data_incomplete: false,
        }];
        let result = classify_surface_type(&assets);
        assert!(result.labels.iter().any(|l| l.contains("permissioned")));
    }

    #[test]
    fn classifies_benji_as_incomplete_when_flagged() {
        let assets = vec![AssetSurfaceInput {
            symbol: "BENJI".into(),
            holder_count: None,
            top10_concentration_pct: None,
            monthly_unique_senders: None,
            monthly_transfer_count: None,
            data_incomplete: true,
        }];
        let result = classify_surface_type(&assets);
        assert!(result
            .labels
            .iter()
            .any(|l| l.contains("incomplete_contract_visibility")));
    }
}
