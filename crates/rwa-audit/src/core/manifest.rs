use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const MANIFEST_VERSION: &str = "1";
pub const EXCHANGE_AUDIT_TITLE: &str = "Exchange — Where RWA Exchange Risk Actually Sits";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditMethod {
    Registry,
    Activity,
    FlowSurface,
    ExchangeSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Verified,
    Partial,
    Gap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManifestClaim {
    pub id: String,
    pub label: String,
    pub value_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_usd: Option<f64>,
    pub as_of: String,
    pub evidence_file: String,
    pub source_url: String,
    pub caveat: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ClaimStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub title: String,
    pub reference_url: String,
    pub frozen_at: String,
    pub panel_date: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<AuditMethod>,
    pub claims: Vec<ManifestClaim>,
    pub do_not_claim: Vec<String>,
}

impl AuditManifest {
    pub fn exchange_template(audit_id: impl Into<String>, frozen_at: String) -> Self {
        Self {
            audit_id: Some(audit_id.into()),
            version: Some(MANIFEST_VERSION.into()),
            title: EXCHANGE_AUDIT_TITLE.into(),
            reference_url:
                "https://egpivo.github.io/2026/06/21/where-rwa-exchange-risk-actually-sits.html"
                    .into(),
            frozen_at,
            panel_date: crate::exchange::config::PUBLISH_PANEL_DATE.into(),
            methods: vec![AuditMethod::ExchangeSurface],
            claims: Vec::new(),
            do_not_claim: vec![
                "Platform transfer ≠ CEX trading volume".into(),
                "Bridged value ≠ transfer volume".into(),
                "Jupiter quote ≠ executed trade or exit capacity".into(),
                "Do not publish rwa_xyz monthly_interpolated extrapolation rows".into(),
                "Do not use exclude_from_interpolation suspect headline ($21.9M)".into(),
            ],
        }
    }

    pub fn claim_ids(&self) -> Vec<&str> {
        self.claims.iter().map(|c| c.id.as_str()).collect()
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(!self.title.is_empty(), "title is required");
        anyhow::ensure!(!self.frozen_at.is_empty(), "frozen_at is required");
        anyhow::ensure!(!self.claims.is_empty(), "at least one claim is required");

        let mut ids = std::collections::HashSet::new();
        for claim in &self.claims {
            anyhow::ensure!(!claim.id.is_empty(), "claim id must not be empty");
            anyhow::ensure!(
                ids.insert(claim.id.as_str()),
                "duplicate claim id: {}",
                claim.id
            );
            anyhow::ensure!(
                !claim.evidence_file.is_empty(),
                "claim {} missing evidence_file",
                claim.id
            );
        }
        Ok(())
    }
}

pub fn load_manifest(path: &Path) -> Result<AuditManifest> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let manifest: AuditManifest =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    manifest.validate()?;
    Ok(manifest)
}

pub fn write_manifest(path: &Path, manifest: &AuditManifest) -> Result<()> {
    manifest.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(manifest)? + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_claim() -> ManifestClaim {
        ManifestClaim {
            id: "test_claim".into(),
            label: "Test claim".into(),
            value_display: "$1".into(),
            value_usd: Some(1.0),
            as_of: "2026-06-12".into(),
            evidence_file: "artifacts/data/test.json".into(),
            source_url: "https://example.com".into(),
            caveat: "fixture".into(),
            status: None,
        }
    }

    #[test]
    fn parses_committed_exchange_manifest() {
        let path = crate::config::repo_root().join("artifacts/data/manifest.json");
        if !path.exists() {
            return;
        }
        let manifest = load_manifest(&path).unwrap();
        assert!(manifest.claims.len() >= 8);
        assert!(!manifest.do_not_claim.is_empty());
    }

    #[test]
    fn exchange_template_has_exchange_method() {
        let m = AuditManifest::exchange_template("article3-xstocks-2026-06-12", "t".into());
        assert_eq!(m.title, EXCHANGE_AUDIT_TITLE);
        assert_eq!(m.methods, vec![AuditMethod::ExchangeSurface]);
        assert_eq!(m.version.as_deref(), Some(MANIFEST_VERSION));
    }

    #[test]
    fn validate_rejects_duplicate_claim_ids() {
        let mut m = AuditManifest::exchange_template("x", "t".into());
        m.claims = vec![fixture_claim(), fixture_claim()];
        assert!(m.validate().is_err());
    }

    #[test]
    fn round_trip_write_and_load() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-manifest-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.json");

        let mut m = AuditManifest::exchange_template("test-audit", "2026-06-17T00:00:00Z".into());
        m.claims.push(fixture_claim());
        write_manifest(&path, &m).unwrap();
        let loaded = load_manifest(&path).unwrap();
        assert_eq!(loaded, m);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deserializes_legacy_manifest_without_new_fields() {
        let json = r#"{
          "title": "Exchange — Where RWA Exchange Risk Actually Sits",
          "reference_url": "https://example.com",
          "frozen_at": "2026-06-17T00:00:00Z",
          "panel_date": "2026-06-12",
          "claims": [{
            "id": "a",
            "label": "l",
            "value_display": "1",
            "value_usd": 1.0,
            "as_of": "2026-06-12",
            "evidence_file": "f",
            "source_url": "u",
            "caveat": "c"
          }],
          "do_not_claim": ["x"]
        }"#;
        let m: AuditManifest = serde_json::from_str(json).unwrap();
        assert!(m.audit_id.is_none());
        assert!(m.methods.is_empty());
        m.validate().unwrap();
    }
}
