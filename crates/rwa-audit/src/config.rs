use std::path::{Path, PathBuf};

pub const SLEEP_BETWEEN_API_MS: u64 = 500;
pub const CHUNK_BLOCKS: u64 = 40_000;
pub const MONTHS_HISTORY: u64 = 6;

pub const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
pub const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

pub const TOTAL_SUPPLY_SEL: &str = "0x18160ddd";

pub fn block_time_secs(chain: &str) -> u64 {
    if chain == "Ethereum" {
        12
    } else {
        2
    }
}

/// Resolve repo root from CARGO_MANIFEST_DIR (crates/rwa-audit) or current dir.
/// Tests may override with `RWA_AUDIT_REPO_ROOT`.
pub fn repo_root() -> PathBuf {
    if let Ok(root) = std::env::var("RWA_AUDIT_REPO_ROOT") {
        return PathBuf::from(root);
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let p = PathBuf::from(manifest);
        if p.ends_with("crates/rwa-audit") {
            return p
                .parent()
                .and_then(|p| p.parent())
                .unwrap_or(&p)
                .to_path_buf();
        }
        return p;
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn data_dir() -> PathBuf {
    repo_root().join("data")
}

pub fn artifacts_data_dir() -> PathBuf {
    repo_root().join("artifacts/data")
}

pub fn audits_dir() -> PathBuf {
    repo_root().join("artifacts/audits")
}

pub fn cache_dir() -> PathBuf {
    repo_root().join("cache")
}

pub fn config_dir() -> PathBuf {
    repo_root().join("config")
}

pub fn path_to_repo_relative(path: &Path) -> String {
    let root = repo_root();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn ensure_dir(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_time_secs_by_chain() {
        assert_eq!(block_time_secs("Ethereum"), 12);
        assert_eq!(block_time_secs("Polygon"), 2);
    }

    #[test]
    fn repo_root_from_manifest_dir_points_at_workspace() {
        let root = repo_root();
        assert!(root.join("Cargo.toml").exists());
        assert!(root.join("crates/rwa-audit").exists());
    }

    #[test]
    fn cache_dir_under_repo_root() {
        let root = repo_root();
        assert!(cache_dir().starts_with(&root));
        assert!(cache_dir().ends_with("cache"));
    }

    #[test]
    fn repo_root_via_env_override() {
        std::env::set_var("RWA_AUDIT_REPO_ROOT", "/tmp/test-override");
        let root = repo_root();
        std::env::remove_var("RWA_AUDIT_REPO_ROOT");
        assert_eq!(root, std::path::Path::new("/tmp/test-override"));
    }

    #[test]
    fn path_to_repo_relative_strips_root_prefix() {
        let root = repo_root();
        let abs = root.join("some/file.json");
        let rel = path_to_repo_relative(&abs);
        assert_eq!(rel, "some/file.json");
    }

    #[test]
    fn path_to_repo_relative_falls_back_to_abs() {
        let abs = std::path::Path::new("/tmp/absolute/path.csv");
        let result = path_to_repo_relative(abs);
        assert!(result.contains("tmp"));
        assert!(result.contains("path.csv"));
    }

    #[test]
    fn config_dir_ends_with_config() {
        assert!(config_dir().ends_with("config"));
    }

    #[test]
    fn audits_dir_ends_with_artifacts_audits() {
        assert!(audits_dir().ends_with("artifacts/audits"));
    }

    #[test]
    fn artifacts_data_dir_ends_with_artifacts_data() {
        assert!(artifacts_data_dir().ends_with("artifacts/data"));
    }

    #[test]
    fn data_dir_ends_with_data() {
        assert!(data_dir().ends_with("data"));
    }

    #[test]
    fn ensure_dir_creates_nested_path() {
        let base = std::env::temp_dir().join(format!(
            "rwa-audit-config-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let nested = base.join("a/b");
        ensure_dir(&nested).unwrap();
        assert!(nested.is_dir());
        let _ = std::fs::remove_dir_all(base);
    }
}
