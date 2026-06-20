//! Publish-bundle contract: article-specific layout + manifest policy.

use std::path::Path;

use anyhow::{bail, Result};

use crate::core::manifest::{
    load_manifest, write_manifest, AuditManifest, AuditMethod, ManifestClaim,
};
use crate::exchange::config::PUBLISH_PANEL_DATE;

#[derive(Debug, Clone)]
pub struct AuditBundleSpec {
    pub id: &'static str,
    /// Flat legacy directory (e.g. artifacts/data) to copy from.
    pub legacy_source: &'static str,
    pub manifest_filename: &'static str,
    pub data_files: &'static [&'static str],
    pub figure_files: &'static [&'static str],
}

#[derive(Debug, thiserror::Error)]
pub enum FreezeError {
    #[error("unknown audit bundle: {0}")]
    UnknownAudit(String),
    #[error("missing source file: {0}")]
    MissingSource(std::path::PathBuf),
}

pub const EXCHANGE_BUNDLE: AuditBundleSpec = AuditBundleSpec {
    id: "article3-xstocks-2026-06-12",
    legacy_source: "artifacts/data",
    manifest_filename: "manifest.json",
    data_files: &[
        "depth_vs_volume_panel_publish.csv",
        "depth_vs_volume_panel.csv",
        "rwa_xyz_platform_transfer_snapshots.json",
        "rwa_xyz_platform_snapshots.json",
        "bridged_value_sum.json",
        "rwa-token-timeseries-export-1781314094816.csv",
        "gecko_aaplx_pools.json",
        "gecko_tslax_pools.json",
        "gecko_spyx_pools.json",
        "jupiter_quote_aaplx_100k.json",
    ],
    figure_files: &["xstocks_surface_snapshot.png"],
};

pub const REGISTRY_BUNDLE: AuditBundleSpec = AuditBundleSpec {
    id: "article1-registry-2026-06",
    legacy_source: "data",
    manifest_filename: "manifest.json",
    data_files: &[
        "rwa_asset_registry.csv",
        "rwa_transfer_metrics.csv",
        "rwa_holder_metrics.csv",
        "rwa_mint_burn_metrics.csv",
        "rwa_data_quality_notes.md",
        "rwa_activity_daily_30d.csv",
    ],
    figure_files: &[],
};

pub fn rewrite_evidence_path(path: &str, audit_id: &str) -> String {
    let file_name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.into());
    format!("artifacts/audits/{audit_id}/data/{file_name}")
}

/// Article-specific publish bundle: file layout, manifest materialization, promote gates.
pub trait PublishBundle: Send + Sync {
    fn spec(&self) -> &'static AuditBundleSpec;

    fn id(&self) -> &'static str {
        self.spec().id
    }

    fn legacy_source(&self) -> &'static str {
        self.spec().legacy_source
    }

    fn manifest_filename(&self) -> &'static str {
        self.spec().manifest_filename
    }

    fn data_files(&self) -> &'static [&'static str] {
        self.spec().data_files
    }

    fn figure_files(&self) -> &'static [&'static str] {
        self.spec().figure_files
    }

    /// Load manifest from flat staging and rewrite bundle-relative claim paths.
    fn prepare_manifest(&self, src_root: &Path) -> Result<Option<AuditManifest>>;

    /// Write `manifest.json` when [`prepare_manifest`] returns `None`.
    fn write_generated_manifest(&self, bundle_root: &Path, data_dir: &Path) -> Result<()> {
        let _ = (bundle_root, data_dir);
        bail!(
            "bundle {} requires a prepared manifest from flat staging",
            self.id()
        )
    }

    /// Extra validation before building a version (e.g. panel date, live guard).
    fn validate_before_promote(&self, panel_date: &str, from_live: bool) -> Result<()> {
        let _ = (panel_date, from_live);
        Ok(())
    }

    /// Guard against standalone `freeze promote` for bundles that require an atomic
    /// collect-then-promote run. Default allows it; override to block.
    fn check_standalone_promote_allowed(&self) -> Result<()> {
        Ok(())
    }

    fn frozen_at(&self) -> String {
        std::env::var("RWA_AUDIT_FROZEN_AT").unwrap_or_else(|_| chrono::Utc::now().to_rfc3339())
    }

    fn panel_date(&self, frozen_at: &str) -> String {
        frozen_at.split('T').next().unwrap_or(frozen_at).to_string()
    }

    fn materialize_manifest(
        &self,
        bundle_root: &Path,
        data_dir: &Path,
        prepared: Option<AuditManifest>,
    ) -> Result<()> {
        match prepared {
            Some(manifest) => write_manifest(&bundle_root.join("manifest.json"), &manifest),
            None => self.write_generated_manifest(bundle_root, data_dir),
        }
    }
}

pub struct ExchangePublishBundle;

pub struct RegistryPublishBundle;

impl PublishBundle for ExchangePublishBundle {
    fn spec(&self) -> &'static AuditBundleSpec {
        &EXCHANGE_BUNDLE
    }

    fn prepare_manifest(&self, src_root: &Path) -> Result<Option<AuditManifest>> {
        let legacy_manifest = src_root.join(self.manifest_filename());
        if !legacy_manifest.exists() {
            bail!(FreezeError::MissingSource(legacy_manifest));
        }
        let mut manifest = load_manifest(&legacy_manifest)?;
        self.validate_before_promote(&manifest.panel_date, false)?;
        manifest.audit_id = Some(self.id().into());
        if manifest.version.is_none() {
            manifest.version = Some(crate::core::manifest::MANIFEST_VERSION.into());
        }
        for claim in &mut manifest.claims {
            claim.evidence_file = rewrite_evidence_path(&claim.evidence_file, self.id());
        }
        Ok(Some(manifest))
    }

    fn validate_before_promote(&self, panel_date: &str, from_live: bool) -> Result<()> {
        validate_exchange_promote(self.id(), panel_date, from_live)
    }
}

impl PublishBundle for RegistryPublishBundle {
    fn spec(&self) -> &'static AuditBundleSpec {
        &REGISTRY_BUNDLE
    }

    fn prepare_manifest(&self, _src_root: &Path) -> Result<Option<AuditManifest>> {
        Ok(None)
    }

    /// Article 1 promotion must go through `rwa-audit run article1 --promote`, which
    /// atomically collects and promotes. The generic `freeze promote` path has no
    /// accompanying collector run and cannot supply a chain-derived panel_date.
    fn check_standalone_promote_allowed(&self) -> Result<()> {
        anyhow::bail!(
            "article1-registry bundle cannot be promoted via 'freeze promote'; \
             use 'rwa-audit run article1 --promote' to atomically collect and promote"
        )
    }

    /// Verify that the evidence bundle carries a non-empty, well-formed panel date from the
    /// collector (derived from block timestamps). An empty date means the caller bypassed
    /// `Article1Module` and no chain-derived date is available.
    fn validate_before_promote(&self, panel_date: &str, _from_live: bool) -> Result<()> {
        anyhow::ensure!(
            !panel_date.is_empty(),
            "article1 promote requires a chain-derived panel_date; \
             run 'rwa-audit run article1 --promote' rather than 'freeze promote'"
        );
        let parts: Vec<&str> = panel_date.split('-').collect();
        anyhow::ensure!(
            parts.len() == 3 && parts[0].len() == 4,
            "panel_date must be YYYY-MM-DD, got {panel_date:?}"
        );
        Ok(())
    }

    fn write_generated_manifest(&self, bundle_root: &Path, data_dir: &Path) -> Result<()> {
        let frozen_at = self.frozen_at();
        // panel_date is always read from the collected activity CSV so it reflects
        // the actual on-chain observation window, never the wall clock or CI env.
        let panel_date = read_max_activity_date(data_dir)?;
        let claims = build_registry_claims(self.id(), data_dir, &panel_date)?;
        let manifest = AuditManifest {
            audit_id: Some(self.id().into()),
            version: Some(crate::core::manifest::MANIFEST_VERSION.into()),
            title: "Registry — If Everything Can Be Tokenized, What Should We Audit?".into(),
            reference_url: "https://egpivo.github.io/2026/06/07/if-everything-can-be-tokenized-what-should-we-audit.html".into(),
            frozen_at,
            panel_date,
            methods: vec![AuditMethod::Registry, AuditMethod::Activity],
            claims,
            do_not_claim: vec![
                "On-chain transfer volume is not CEX trading volume".into(),
                "Holder concentration from Ethplorer may omit permissioned counterparties".into(),
            ],
        };
        write_manifest(&bundle_root.join("manifest.json"), &manifest)
    }
}

static EXCHANGE_PUBLISH: ExchangePublishBundle = ExchangePublishBundle;
static REGISTRY_PUBLISH: RegistryPublishBundle = RegistryPublishBundle;

pub fn resolve_publish_bundle(audit_id: &str) -> Result<&'static dyn PublishBundle, FreezeError> {
    if audit_id == EXCHANGE_BUNDLE.id {
        Ok(&EXCHANGE_PUBLISH)
    } else if audit_id == REGISTRY_BUNDLE.id {
        Ok(&REGISTRY_PUBLISH)
    } else {
        Err(FreezeError::UnknownAudit(audit_id.into()))
    }
}

pub fn list_publish_bundles() -> [&'static dyn PublishBundle; 2] {
    [&EXCHANGE_PUBLISH, &REGISTRY_PUBLISH]
}

/// Panel date encoded in a versioned exchange bundle id.
pub fn exchange_bundle_panel_date(audit_id: &str) -> Option<&'static str> {
    if audit_id == EXCHANGE_BUNDLE.id {
        Some(PUBLISH_PANEL_DATE)
    } else {
        None
    }
}

/// Guard against promoting live staging or misdated evidence into a fixed bundle.
pub fn validate_exchange_promote(bundle_id: &str, panel_date: &str, from_live: bool) -> Result<()> {
    if from_live {
        anyhow::bail!(
            "cannot promote live exchange evidence into publish bundle {bundle_id}; \
             review data/exchange-live/ and run an offline freeze to artifacts/data/ before promote"
        );
    }
    if let Some(expected) = exchange_bundle_panel_date(bundle_id) {
        anyhow::ensure!(
            panel_date == expected,
            "panel date {panel_date} does not match bundle id {bundle_id} (expected {expected})"
        );
    }
    Ok(())
}

/// Read the latest observation date from the collected activity CSV.
///
/// The `date` column is the first column (YYYY-MM-DD strings); lexicographic max equals
/// the chronological max. This is the only source of truth for the manifest's `panel_date`
/// and claim `as_of` fields — never derived from wall clock or `RWA_AUDIT_FROZEN_AT`.
fn read_max_activity_date(data_dir: &Path) -> Result<String> {
    use anyhow::Context as _;
    let path = data_dir.join("rwa_activity_daily_30d.csv");
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("read activity CSV {}", path.display()))?;
    let max_date = content
        .lines()
        .skip(1) // header row
        .filter_map(|line| line.split(',').next())
        .filter(|s| !s.trim().is_empty())
        .max()
        .map(str::to_string)
        .with_context(|| format!("activity CSV {} has no data rows", path.display()))?;
    Ok(max_date)
}

fn build_registry_claims(
    audit_id: &str,
    data_dir: &Path,
    as_of: &str,
) -> Result<Vec<ManifestClaim>> {
    let artifacts = [
        (
            "registry_universe",
            "rwa_asset_registry.csv",
            "Article 1 ERC-20 registry universe",
        ),
        (
            "transfer_metrics",
            "rwa_transfer_metrics.csv",
            "Monthly ERC-20 transfer metrics",
        ),
        (
            "holder_metrics",
            "rwa_holder_metrics.csv",
            "Holder concentration snapshot",
        ),
        (
            "mint_burn_metrics",
            "rwa_mint_burn_metrics.csv",
            "Mint and burn activity metrics",
        ),
        (
            "activity_timeseries",
            "rwa_activity_daily_30d.csv",
            "30-day observable activity time series",
        ),
    ];

    let mut claims = Vec::new();
    for (id, file, label) in artifacts {
        let path = data_dir.join(file);
        anyhow::ensure!(
            path.exists(),
            "missing registry evidence file: {}",
            path.display()
        );
        let rows = count_csv_data_rows(&path)?;
        claims.push(ManifestClaim {
            id: id.into(),
            label: format!("{label} ({rows} rows)"),
            value_display: format!("{rows} rows"),
            value_usd: None,
            as_of: as_of.into(),
            evidence_file: format!("artifacts/audits/{audit_id}/data/{file}"),
            source_url: "https://github.com/egpivo/rwa-audit".into(),
            caveat: "Promoted from live collection outputs in data/.".into(),
            status: None,
        });
    }
    Ok(claims)
}

fn count_csv_data_rows(path: &Path) -> Result<usize> {
    let text = std::fs::read_to_string(path)?;
    let rows = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        .saturating_sub(1);
    Ok(rows)
}
