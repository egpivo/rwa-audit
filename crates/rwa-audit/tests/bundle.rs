//! Integration tests for audit bundle promotion (isolated temp repo root).

use std::path::Path;

use rwa_audit::core::bundle::{
    bundle_data_dir_at, bundle_manifest_path_at, promote_audit_bundle_at, EXCHANGE_BUNDLE,
    REGISTRY_BUNDLE,
};
use rwa_audit::core::manifest::load_manifest;

struct TempRepo {
    root: std::path::PathBuf,
}

impl TempRepo {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rwa-audit-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn seed_exchange_publish_from_workspace(&self) {
        let workspace =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../artifacts/data");
        if !workspace.join("manifest.json").exists() {
            return;
        }
        let dest = self.root.join("artifacts/data");
        std::fs::create_dir_all(&dest).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace.join(name);
            if src.exists() {
                std::fs::copy(src, dest.join(name)).unwrap();
            }
        }
        std::fs::copy(workspace.join("manifest.json"), dest.join("manifest.json")).unwrap();
        let figures = self.root.join("artifacts/figures");
        std::fs::create_dir_all(&figures).unwrap();
        let fig_src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/figures/xstocks_surface_snapshot.png");
        if fig_src.exists() {
            std::fs::copy(fig_src, figures.join("xstocks_surface_snapshot.png")).unwrap();
        }
    }

    fn seed_registry_from_workspace(&self) -> bool {
        let workspace_data =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        if !workspace_data.join("rwa_asset_registry.csv").exists() {
            return false;
        }
        let dest = self.root.join("data");
        std::fs::create_dir_all(&dest).unwrap();
        for name in REGISTRY_BUNDLE.data_files {
            let src = workspace_data.join(name);
            if !src.exists() {
                return false;
            }
            std::fs::copy(src, dest.join(name)).unwrap();
        }
        true
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn workspace_has_exchange_publish() -> bool {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/data/manifest.json")
        .exists()
}

#[test]
fn promote_exchange_bundle_writes_bundle_layout_in_temp_repo() {
    if !workspace_has_exchange_publish() {
        return;
    }

    let temp = TempRepo::new();
    temp.seed_exchange_publish_from_workspace();

    let bundle_root = promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &temp.root)
        .expect("promote exchange bundle in temp repo");
    assert_eq!(
        bundle_root,
        bundle_manifest_path_at(EXCHANGE_BUNDLE.id, &temp.root)
            .parent()
            .unwrap()
    );
    assert!(bundle_data_dir_at(EXCHANGE_BUNDLE.id, &temp.root)
        .join("depth_vs_volume_panel_publish.csv")
        .exists());
    assert!(bundle_manifest_path_at(EXCHANGE_BUNDLE.id, &temp.root).exists());
    assert!(
        !bundle_data_dir_at(EXCHANGE_BUNDLE.id, &temp.root)
            .join("manifest.json")
            .exists(),
        "bundle data/ must not duplicate root manifest.json"
    );

    let manifest = load_manifest(&bundle_manifest_path_at(EXCHANGE_BUNDLE.id, &temp.root)).unwrap();
    assert_eq!(manifest.audit_id.as_deref(), Some(EXCHANGE_BUNDLE.id));
    assert!(manifest.article.contains("Exchange"));
    assert!(manifest.claims[0]
        .evidence_file
        .contains("artifacts/audits/"));
}

#[test]
fn promote_registry_bundle_writes_valid_manifest_in_temp_repo() {
    let temp = TempRepo::new();
    if !temp.seed_registry_from_workspace() {
        return;
    }

    promote_audit_bundle_at(REGISTRY_BUNDLE.id, &temp.root)
        .expect("promote registry bundle in temp repo");
    let manifest = load_manifest(&bundle_manifest_path_at(REGISTRY_BUNDLE.id, &temp.root)).unwrap();
    assert_eq!(manifest.audit_id.as_deref(), Some(REGISTRY_BUNDLE.id));
    assert!(!manifest.claims.is_empty());
    assert!(manifest
        .claims
        .iter()
        .all(|c| c.evidence_file.contains("artifacts/audits/")));
}
