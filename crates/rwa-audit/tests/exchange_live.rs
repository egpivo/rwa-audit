//! Optional live-network exchange freeze checks (not run in CI).

use rwa_audit::{AuditContext, RunMode};

#[test]
#[ignore = "hits GeckoTerminal and Jupiter live APIs"]
fn live_exchange_freeze_writes_staging_evidence() {
    use std::path::PathBuf;

    let root = std::env::temp_dir().join(format!(
        "rwa-exchange-live-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("artifacts/data")).unwrap();
    let publish = rwa_audit::config::repo_root().join("artifacts/data");
    for name in [
        "rwa_xyz_platform_transfer_snapshots.json",
        "rwa-token-timeseries-export-1781314094816.csv",
    ] {
        let src = publish.join(name);
        if src.exists() {
            std::fs::copy(src, root.join("artifacts/data").join(name)).unwrap();
        }
    }

    let prev = std::env::var("RWA_AUDIT_REPO_ROOT").ok();
    std::env::set_var("RWA_AUDIT_REPO_ROOT", &root);
    let result = (|| -> anyhow::Result<PathBuf> {
        let ctx = AuditContext::new()?;
        rwa_audit::run_module(
            "exchange",
            &ctx,
            RunMode::Live,
            &rwa_audit::audit::RunExtra::default(),
        )?;
        let out_dir = rwa_audit::exchange::config::exchange_live_staging_dir();
        assert!(out_dir.join("manifest.json").exists());
        assert!(out_dir
            .join("rwa_xyz_platform_transfer_snapshots.json")
            .exists());
        Ok(out_dir)
    })();
    restore_repo_root(prev);
    let out_dir = result.expect("live exchange freeze");
    let _ = std::fs::remove_dir_all(root);
    let _ = out_dir;
}

fn restore_repo_root(prev: Option<String>) {
    match prev {
        Some(value) => std::env::set_var("RWA_AUDIT_REPO_ROOT", value),
        None => std::env::remove_var("RWA_AUDIT_REPO_ROOT"),
    }
}
