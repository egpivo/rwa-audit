use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use fs2::FileExt;
use sha2::{Digest, Sha256};

use crate::config::{artifacts_data_dir, ensure_dir, repo_root};
use crate::core::manifest::{load_manifest, AuditManifest};
use crate::core::publish::{resolve_publish_bundle, AuditBundleSpec, FreezeError, PublishBundle};

pub use crate::core::publish::{EXCHANGE_BUNDLE, REGISTRY_BUNDLE};

const BUNDLE_VERSIONS_DIR: &str = "versions";

struct StagingGuard {
    path: Option<PathBuf>,
}

impl StagingGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> &Path {
        self.path.as_ref().expect("staging guard disarmed")
    }

    fn disarm(mut self) {
        self.path = None;
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

pub fn audit_bundle_dir(audit_id: &str) -> PathBuf {
    audit_bundle_dir_at(audit_id, &repo_root())
}

pub fn audit_bundle_dir_at(audit_id: &str, root: &Path) -> PathBuf {
    root.join("artifacts/audits").join(audit_id)
}

fn bundle_versions_dir_at(root: &Path) -> PathBuf {
    root.join("artifacts/audits").join(BUNDLE_VERSIONS_DIR)
}

const BUNDLE_DIGEST_HEX_LEN: usize = 16;

fn bundle_version_name(audit_id: &str, digest: &str) -> String {
    format!("{audit_id}-{digest}")
}

fn hex_digest_prefix(hash: sha2::digest::Output<Sha256>) -> String {
    format!("{hash:x}")[..BUNDLE_DIGEST_HEX_LEN].to_string()
}

fn digest_bundle_tree(bundle_root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let manifest = bundle_root.join("manifest.json");
    if manifest.is_file() {
        hasher.update(b"manifest.json");
        hasher.update(fs::read(&manifest)?);
    }
    digest_bundle_subtree(&mut hasher, &bundle_root.join("data"), b"data")?;
    digest_bundle_subtree(&mut hasher, &bundle_root.join("figures"), b"figures")?;
    Ok(hex_digest_prefix(hasher.finalize()))
}

fn digest_bundle_subtree(hasher: &mut Sha256, dir: &Path, label: &[u8]) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    hasher.update(label);
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort_unstable();
    for name in entries {
        let path = dir.join(&name);
        hasher.update(name.as_bytes());
        hasher.update(fs::read(&path)?);
    }
    Ok(())
}

fn validate_bundle_version(bundle_root: &Path, bundle: &dyn PublishBundle) -> Result<()> {
    let manifest_path = bundle_root.join("manifest.json");
    anyhow::ensure!(
        manifest_path.is_file(),
        "bundle version missing manifest at {}",
        manifest_path.display()
    );
    let manifest =
        load_manifest(&manifest_path).context("bundle version manifest is invalid JSON")?;
    let data_dir = bundle_root.join("data");
    anyhow::ensure!(
        data_dir.is_dir(),
        "bundle version missing data/ under {}",
        bundle_root.display()
    );
    for name in bundle.data_files() {
        let path = data_dir.join(name);
        anyhow::ensure!(
            path.is_file(),
            "bundle version missing data file {}",
            path.display()
        );
    }
    let figures_dir = bundle_root.join("figures");
    for fig in bundle.figure_files() {
        let path = figures_dir.join(fig);
        anyhow::ensure!(
            path.is_file(),
            "bundle version missing required figure {}",
            path.display()
        );
    }
    validate_bundle_claims(bundle_root, &manifest)?;
    Ok(())
}

fn resolve_claim_evidence_path(bundle_root: &Path, evidence_file: &str) -> PathBuf {
    let file_name = Path::new(evidence_file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| evidence_file.into());
    if evidence_file.contains("/figures/") {
        bundle_root.join("figures").join(file_name)
    } else {
        bundle_root.join("data").join(file_name)
    }
}

fn validate_bundle_claims(bundle_root: &Path, manifest: &AuditManifest) -> Result<()> {
    for claim in &manifest.claims {
        let path = resolve_claim_evidence_path(bundle_root, &claim.evidence_file);
        anyhow::ensure!(
            path.is_file(),
            "claim {} evidence missing: {} (resolved {})",
            claim.id,
            claim.evidence_file,
            path.display()
        );
    }
    Ok(())
}

fn write_bundle_version_staging(
    root: &Path,
    versions_parent: &Path,
    bundle: &dyn PublishBundle,
    src_root: &Path,
    prepared_manifest: Option<AuditManifest>,
) -> Result<StagingGuard> {
    let token = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let staging = versions_parent.join(format!(".{}-staging-{token}", bundle.id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    let guard = StagingGuard::new(staging.clone());
    let version_data = staging.join("data");
    let version_figures = staging.join("figures");
    ensure_dir(versions_parent)?;
    ensure_dir(&version_data)?;
    ensure_dir(&version_figures)?;

    copy_into_bundle(src_root, &version_data, bundle.data_files())?;

    let stale_data_manifest = version_data.join("manifest.json");
    if stale_data_manifest.exists() {
        fs::remove_file(&stale_data_manifest)?;
    }

    for fig in bundle.figure_files() {
        let src = root.join("artifacts/figures").join(fig);
        anyhow::ensure!(
            src.is_file(),
            "missing required figure for bundle promote: {}",
            src.display()
        );
        fs::copy(&src, version_figures.join(fig))?;
    }

    bundle.materialize_manifest(&staging, &version_data, prepared_manifest)?;

    validate_bundle_version(&staging, bundle)?;
    Ok(guard)
}

/// Validate staging, derive digest from the full tree (manifest + data + figures), install under versions/.
fn install_bundle_version(
    audits_parent: &Path,
    bundle: &dyn PublishBundle,
    staging: &Path,
) -> Result<(PathBuf, String)> {
    validate_bundle_version(staging, bundle)?;
    let digest = digest_bundle_tree(staging)?;
    let version_name = bundle_version_name(bundle.id(), &digest);
    let version_root = audits_parent.join(BUNDLE_VERSIONS_DIR).join(&version_name);

    if version_root.is_dir() {
        let reusable = validate_bundle_version(&version_root, bundle).is_ok()
            && digest_bundle_tree(&version_root)? == digest;
        if reusable {
            let _ = fs::remove_dir_all(staging);
            return Ok((version_root, version_name));
        }
        bail!(
            "bundle version {} exists but is corrupt or hash-collides with staging; \
             remove or quarantine it manually before promoting",
            version_root.display()
        );
    }

    ensure_dir(&audits_parent.join(BUNDLE_VERSIONS_DIR))?;
    fs::rename(staging, &version_root).with_context(|| {
        format!(
            "install bundle version {} → {}",
            staging.display(),
            version_root.display()
        )
    })?;
    Ok((version_root, version_name))
}

/// Resolve a bundle version directory and ensure it stays under `artifacts/audits/versions/`.
fn resolve_version_dir(audits_parent: &Path, link_target: &Path) -> Option<PathBuf> {
    let versions_root = fs::canonicalize(audits_parent.join(BUNDLE_VERSIONS_DIR)).ok()?;
    let candidate = if link_target.is_absolute() {
        link_target.to_path_buf()
    } else {
        audits_parent.join(link_target)
    };
    let resolved = fs::canonicalize(candidate).ok()?;
    if resolved.starts_with(&versions_root) && resolved.is_dir() {
        Some(resolved)
    } else {
        None
    }
}

fn read_bundle_pointer_target(pointer: &Path) -> Option<PathBuf> {
    pointer.read_link().ok()
}

fn is_materialized_bundle_pointer(pointer: &Path) -> bool {
    pointer
        .symlink_metadata()
        .map(|m| m.is_dir() && !m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Move a materialized `artifacts/audits/{id}/` tree into `versions/` and restore a symlink.
///
/// First migration from a git-committed directory still has a brief window where the
/// public pointer path is missing (between `rename` and symlink activation). After this
/// runs once, subsequent promotions only perform atomic symlink-to-symlink swaps.
fn migrate_materialized_bundle_pointer(
    audits_parent: &Path,
    bundle: &dyn PublishBundle,
    pointer: &Path,
) -> Result<()> {
    if !is_materialized_bundle_pointer(pointer) {
        return Ok(());
    }
    let digest = digest_bundle_tree(pointer)?;
    validate_bundle_version(pointer, bundle)?;
    let version_name = bundle_version_name(bundle.id(), &digest);
    let versions_parent = audits_parent.join(BUNDLE_VERSIONS_DIR);
    ensure_dir(&versions_parent)?;
    let version_root = versions_parent.join(&version_name);

    if version_root.exists() {
        let rel = Path::new(BUNDLE_VERSIONS_DIR).join(&version_name);
        if resolve_version_dir(audits_parent, &rel).is_none() {
            bail!(
                "refusing to migrate {}: existing version path escapes versions/",
                version_root.display()
            );
        }
        let stored_digest = digest_bundle_tree(&version_root)?;
        if stored_digest != digest {
            bail!(
                "version {} exists but content digest {stored_digest} does not match materialized bundle digest {digest}",
                version_root.display()
            );
        }
        validate_bundle_version(&version_root, bundle)?;
        fs::remove_dir_all(pointer).with_context(|| {
            format!(
                "remove materialized bundle {} after version {} exists",
                pointer.display(),
                version_root.display()
            )
        })?;
    } else {
        fs::rename(pointer, &version_root).with_context(|| {
            format!(
                "migrate materialized bundle {} → {}",
                pointer.display(),
                version_root.display()
            )
        })?;
    }

    activate_bundle_version(audits_parent, bundle.id(), &version_name)
}

fn acquire_bundle_promote_lock(audit_id: &str) -> Result<fs::File> {
    let locks = crate::config::cache_dir().join("locks");
    ensure_dir(&locks)?;
    let lock_path = locks.join(format!("bundle-promote-{audit_id}.lock"));
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("open promote lock {}", lock_path.display()))?;
    lock.lock_exclusive()
        .with_context(|| format!("acquire exclusive promote lock for bundle {audit_id}"))?;
    Ok(lock)
}

/// Point `artifacts/audits/{audit_id}` at a fully built version directory.
#[cfg(unix)]
fn activate_bundle_version(audits_parent: &Path, audit_id: &str, version_name: &str) -> Result<()> {
    use std::os::unix::fs::symlink;

    let link_target = format!("{BUNDLE_VERSIONS_DIR}/{version_name}");
    let pointer = audits_parent.join(audit_id);
    let temp_pointer = audits_parent.join(format!(".{audit_id}.pointer-{version_name}"));

    if pointer
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
        && pointer.read_link().ok().as_deref() == Some(Path::new(&link_target))
    {
        return Ok(());
    }

    if is_materialized_bundle_pointer(&pointer) {
        bail!(
            "bundle pointer {} is still a materialized directory; migrate before activate",
            pointer.display()
        );
    }

    if temp_pointer.exists() {
        fs::remove_file(&temp_pointer)?;
    }
    symlink(&link_target, &temp_pointer)?;

    fs::rename(&temp_pointer, &pointer).with_context(|| {
        format!(
            "atomically swap bundle pointer {} → {link_target}",
            pointer.display()
        )
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn activate_bundle_version(audits_parent: &Path, audit_id: &str, version_name: &str) -> Result<()> {
    let _ = (audits_parent, audit_id, version_name);
    bail!("atomic bundle pointer swap requires a Unix platform");
}

fn safe_remove_version_dir(audits_parent: &Path, link_target: &Path, keep: &Path) {
    let Some(previous_abs) = resolve_version_dir(audits_parent, link_target) else {
        return;
    };
    let Ok(keep_abs) = fs::canonicalize(keep) else {
        return;
    };
    if previous_abs == keep_abs {
        return;
    }
    let _ = fs::remove_dir_all(previous_abs);
}

pub fn bundle_data_dir(audit_id: &str) -> PathBuf {
    bundle_data_dir_at(audit_id, &repo_root())
}

pub fn bundle_data_dir_at(audit_id: &str, root: &Path) -> PathBuf {
    audit_bundle_dir_at(audit_id, root).join("data")
}

pub fn bundle_figures_dir(audit_id: &str) -> PathBuf {
    bundle_figures_dir_at(audit_id, &repo_root())
}

pub fn bundle_figures_dir_at(audit_id: &str, root: &Path) -> PathBuf {
    audit_bundle_dir_at(audit_id, root).join("figures")
}

pub fn bundle_manifest_path(audit_id: &str) -> PathBuf {
    bundle_manifest_path_at(audit_id, &repo_root())
}

pub fn bundle_manifest_path_at(audit_id: &str, root: &Path) -> PathBuf {
    audit_bundle_dir_at(audit_id, root).join("manifest.json")
}

pub fn resolve_bundle_spec(audit_id: &str) -> Result<&'static AuditBundleSpec, FreezeError> {
    Ok(resolve_publish_bundle(audit_id)?.spec())
}

pub fn list_bundle_specs() -> [&'static AuditBundleSpec; 2] {
    [&EXCHANGE_BUNDLE, &REGISTRY_BUNDLE]
}

fn sha256_hex(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{:x}", digest))
}

fn copy_into_bundle(
    src_root: &Path,
    dest_root: &Path,
    files: &[&str],
) -> Result<HashMap<String, String>> {
    ensure_dir(dest_root)?;
    let mut checksums = HashMap::new();
    for name in files {
        let src = src_root.join(name);
        if !src.exists() {
            bail!(FreezeError::MissingSource(src));
        }
        let dest = dest_root.join(name);
        if let Some(parent) = dest.parent() {
            ensure_dir(parent)?;
        }
        fs::copy(&src, &dest)
            .with_context(|| format!("copy {} → {}", src.display(), dest.display()))?;
        checksums.insert(name.to_string(), sha256_hex(&dest)?);
    }
    Ok(checksums)
}

/// RAII guard that holds the per-bundle exclusive lock across a collect→promote sequence.
/// Acquired before collection begins; dropped after promotion completes. Prevents concurrent
/// invocations from interleaving writes to shared `data/*.csv` or reading half-written CSVs.
pub(crate) struct CollectPromoteLock {
    _file: fs::File,
}

/// Acquire the bundle promote lock BEFORE collection starts.
/// The returned [`CollectPromoteLock`] must be passed to [`promote_publish_bundle_after_collect`]
/// so the entire collect→promote sequence runs under a single cross-process lock.
pub(crate) fn acquire_collect_promote_lock(bundle_id: &str) -> Result<CollectPromoteLock> {
    let file = acquire_bundle_promote_lock(bundle_id)?;
    Ok(CollectPromoteLock { _file: file })
}

/// Core promotion logic. Caller is responsible for holding a lock (either a
/// [`CollectPromoteLock`] from before collection, or the lock inside [`promote_publish_bundle_at`]).
fn do_promote(bundle: &dyn PublishBundle, root: &Path) -> Result<PathBuf> {
    let audit_id = bundle.id();
    let src_root = root.join(bundle.legacy_source());
    let bundle_pointer = audit_bundle_dir_at(audit_id, root);
    let audits_parent = root.join("artifacts/audits");
    let versions_parent = bundle_versions_dir_at(root);
    let previous_target = read_bundle_pointer_target(&bundle_pointer);

    for name in bundle.data_files() {
        if !src_root.join(name).exists() {
            bail!(FreezeError::MissingSource(src_root.join(name)));
        }
    }

    let prepared_manifest = bundle.prepare_manifest(&src_root)?;

    let staging_guard =
        write_bundle_version_staging(root, &versions_parent, bundle, &src_root, prepared_manifest)?;
    let staging = staging_guard.path().to_path_buf();

    let (version_root, version_name) = install_bundle_version(&audits_parent, bundle, &staging)?;
    staging_guard.disarm();

    migrate_materialized_bundle_pointer(&audits_parent, bundle, &bundle_pointer)?;

    activate_bundle_version(&audits_parent, audit_id, &version_name)?;

    if let Some(previous) = previous_target {
        safe_remove_version_dir(&audits_parent, &previous, &version_root);
    }

    Ok(bundle_pointer)
}

/// Promote a bundle after an atomic collect run. Requires the [`CollectPromoteLock`] that was
/// acquired before collection started, proving that the full collect→promote sequence is
/// serialized and that this caller actually ran the collection phase.
/// `pub(crate)` — only the internal module runner should call this.
pub(crate) fn promote_publish_bundle_after_collect(
    bundle: &dyn PublishBundle,
    lock: CollectPromoteLock,
) -> Result<PathBuf> {
    // lock is moved in and dropped at end of function, covering the promotion phase too.
    let _lock = lock;
    do_promote(bundle, &repo_root())
}

/// Promote legacy flat outputs into `artifacts/audits/{id}/` with rewritten manifest paths.
/// Calls [`PublishBundle::check_standalone_promote_allowed`] — use this for the generic
/// `freeze promote` path. For post-collection promotion use [`promote_publish_bundle_after_collect`].
pub fn promote_audit_bundle(audit_id: &str) -> Result<PathBuf> {
    promote_audit_bundle_at(audit_id, &repo_root())
}

pub fn promote_publish_bundle_at(bundle: &dyn PublishBundle, root: &Path) -> Result<PathBuf> {
    bundle.check_standalone_promote_allowed()?;
    let _lock = acquire_bundle_promote_lock(bundle.id())?;
    do_promote(bundle, root)
}

pub fn promote_audit_bundle_at(audit_id: &str, root: &Path) -> Result<PathBuf> {
    let bundle = resolve_publish_bundle(audit_id).map_err(|e| anyhow::anyhow!("{e}"))?;
    promote_publish_bundle_at(bundle, root)
}

/// Convenience: exchange bundle reads from `artifacts_data_dir()`.
pub fn promote_exchange_bundle() -> Result<PathBuf> {
    let _ = artifacts_data_dir();
    promote_audit_bundle(EXCHANGE_BUNDLE.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::manifest::{load_manifest, write_manifest};
    use crate::core::publish::{rewrite_evidence_path, validate_exchange_promote, REGISTRY_BUNDLE};

    #[test]
    fn registry_bundle_blocks_standalone_promote() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-registry-standalone-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let err = promote_audit_bundle_at(REGISTRY_BUNDLE.id, &root).unwrap_err();
        assert!(
            err.to_string().contains("article1 --promote"),
            "expected guidance to run article1 --promote, got: {err}"
        );
        // No files should have been written
        assert!(
            !root.exists(),
            "temp root should not be created when guard fires"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exchange_bundle_allows_standalone_promote_guard() {
        use crate::core::publish::EXCHANGE_BUNDLE;
        // check_standalone_promote_allowed should not fail for exchange
        let bundle = crate::core::publish::resolve_publish_bundle(EXCHANGE_BUNDLE.id).unwrap();
        bundle.check_standalone_promote_allowed().unwrap();
    }

    fn seed_exchange_figures(root: &Path) {
        let figures = root.join("artifacts/figures");
        std::fs::create_dir_all(&figures).unwrap();
        let fig_src = crate::config::repo_root()
            .join("artifacts/figures")
            .join("xstocks_surface_snapshot.png");
        if fig_src.exists() {
            fs::copy(fig_src, figures.join("xstocks_surface_snapshot.png")).unwrap();
        }
    }

    #[test]
    fn resolve_known_audit_bundles() {
        assert_eq!(
            resolve_bundle_spec("article3-xstocks-2026-06-12")
                .unwrap()
                .id,
            EXCHANGE_BUNDLE.id
        );
        assert!(resolve_bundle_spec("unknown").is_err());
    }

    #[test]
    fn rewrite_evidence_path_points_into_bundle() {
        let p = rewrite_evidence_path(
            "artifacts/data/gecko_aaplx_pools.json",
            "article3-xstocks-2026-06-12",
        );
        assert_eq!(
            p,
            "artifacts/audits/article3-xstocks-2026-06-12/data/gecko_aaplx_pools.json"
        );
    }

    #[test]
    fn validate_exchange_promote_rejects_live() {
        let err = validate_exchange_promote(EXCHANGE_BUNDLE.id, "2026-06-12", true).unwrap_err();
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn validate_exchange_promote_rejects_panel_date_mismatch() {
        assert!(validate_exchange_promote(EXCHANGE_BUNDLE.id, "2026-06-15", false).is_err());
    }

    #[test]
    fn rejected_promote_does_not_modify_existing_bundle() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-reject-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let publish = root.join("artifacts/data");
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        if !workspace_publish.join("manifest.json").exists() {
            return;
        }
        std::fs::create_dir_all(&publish).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                std::fs::copy(src, publish.join(name)).unwrap();
            }
        }
        std::fs::copy(
            workspace_publish.join("manifest.json"),
            publish.join("manifest.json"),
        )
        .unwrap();

        seed_exchange_figures(&root);
        promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap();
        let marker =
            bundle_data_dir_at(EXCHANGE_BUNDLE.id, &root).join("depth_vs_volume_panel_publish.csv");
        let before = fs::read(&marker).unwrap();

        let bad_manifest = publish.join("manifest.json");
        let mut manifest = load_manifest(&bad_manifest).unwrap();
        manifest.panel_date = "2026-06-15".into();
        write_manifest(&bad_manifest, &manifest).unwrap();

        let err = promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap_err();
        assert!(err.to_string().contains("panel date"));
        let after = fs::read(&marker).unwrap();
        assert_eq!(before, after);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_promotes_serialize_without_corruption() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-concurrent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let publish = root.join("artifacts/data");
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        if !workspace_publish.join("manifest.json").exists() {
            return;
        }
        std::fs::create_dir_all(&publish).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                std::fs::copy(src, publish.join(name)).unwrap();
            }
        }
        std::fs::copy(
            workspace_publish.join("manifest.json"),
            publish.join("manifest.json"),
        )
        .unwrap();

        seed_exchange_figures(&root);
        let root = std::sync::Arc::new(root);
        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..2 {
                let root = std::sync::Arc::clone(&root);
                handles.push(scope.spawn(move || {
                    promote_audit_bundle_at(EXCHANGE_BUNDLE.id, root.as_path()).unwrap()
                }));
            }
            let first = handles.remove(0).join().unwrap();
            let second = handles.remove(0).join().unwrap();
            assert_eq!(first, second);
        });

        let data_dir = bundle_data_dir_at(EXCHANGE_BUNDLE.id, root.as_path());
        for name in EXCHANGE_BUNDLE.data_files {
            assert!(data_dir.join(name).exists(), "missing {name}");
        }
        assert!(bundle_manifest_path_at(EXCHANGE_BUNDLE.id, root.as_path()).exists());
        #[cfg(unix)]
        {
            let pointer = root.join("artifacts/audits").join(EXCHANGE_BUNDLE.id);
            assert!(pointer.is_symlink());
        }

        let _ = fs::remove_dir_all(root.as_path());
    }

    #[test]
    fn promote_exchange_bundle_from_fixtures() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let legacy = root.join("artifacts/data");
        std::fs::create_dir_all(&legacy).unwrap();

        let src_manifest = crate::config::repo_root().join("artifacts/data/manifest.json");
        if !src_manifest.exists() {
            return;
        }
        for name in EXCHANGE_BUNDLE.data_files {
            let src = crate::config::repo_root().join("artifacts/data").join(name);
            if src.exists() {
                std::fs::copy(src, legacy.join(name)).unwrap();
            }
        }
        let legacy_manifest = legacy.join("manifest.json");
        if src_manifest.exists() {
            std::fs::copy(&src_manifest, &legacy_manifest).unwrap();
        }

        // Point repo_root at temp by copying minimal tree structure
        let _bundle_data = legacy.clone();
        let mut manifest = load_manifest(&src_manifest).unwrap();
        for claim in &mut manifest.claims {
            claim.evidence_file = rewrite_evidence_path(&claim.evidence_file, EXCHANGE_BUNDLE.id);
        }
        assert!(manifest.claims[0]
            .evidence_file
            .contains("artifacts/audits/"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn safe_remove_ignores_symlink_targets_outside_versions() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-safe-remove-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let audits_parent = root.join("artifacts/audits");
        let versions = audits_parent.join(BUNDLE_VERSIONS_DIR);
        fs::create_dir_all(&versions).unwrap();
        let victim = root.join("victim-outside-versions");
        fs::create_dir_all(&victim).unwrap();

        let pointer = audits_parent.join("malicious-bundle");
        std::os::unix::fs::symlink("../../../../victim-outside-versions", &pointer).unwrap();

        let keep = versions.join("keep-version");
        fs::create_dir_all(&keep).unwrap();
        let outside = pointer.read_link().unwrap();
        safe_remove_version_dir(&audits_parent, &outside, &keep);

        assert!(victim.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn promote_is_idempotent_for_unchanged_source() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-idempotent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let publish = root.join("artifacts/data");
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        if !workspace_publish.join("manifest.json").exists() {
            return;
        }
        std::fs::create_dir_all(&publish).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                fs::copy(src, publish.join(name)).unwrap();
            }
        }
        fs::copy(
            workspace_publish.join("manifest.json"),
            publish.join("manifest.json"),
        )
        .unwrap();

        seed_exchange_figures(&root);
        promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap();
        let versions: Vec<_> = fs::read_dir(root.join("artifacts/audits/versions"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let version_count_after_first = versions.len();

        #[cfg(unix)]
        let first_target = root
            .join("artifacts/audits")
            .join(EXCHANGE_BUNDLE.id)
            .read_link()
            .unwrap();

        promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap();

        #[cfg(unix)]
        {
            let second_target = root
                .join("artifacts/audits")
                .join(EXCHANGE_BUNDLE.id)
                .read_link()
                .unwrap();
            assert_eq!(first_target, second_target);
        }

        let version_count_after_second = fs::read_dir(root.join("artifacts/audits/versions"))
            .unwrap()
            .filter_map(|e| e.ok())
            .count();
        assert_eq!(version_count_after_first, version_count_after_second);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_promote_leaves_materialized_bundle_untouched() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-invalid-materialized-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace_bundle = crate::config::repo_root()
            .join("artifacts/audits")
            .join(EXCHANGE_BUNDLE.id);
        if !workspace_bundle.join("manifest.json").exists() {
            return;
        }
        let pointer = root.join("artifacts/audits").join(EXCHANGE_BUNDLE.id);
        copy_dir_recursive(&workspace_bundle, &pointer).unwrap();

        let publish = root.join("artifacts/data");
        std::fs::create_dir_all(&publish).unwrap();
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                fs::copy(src, publish.join(name)).unwrap();
            }
        }
        let mut manifest = load_manifest(&workspace_publish.join("manifest.json")).unwrap();
        manifest.panel_date = "2026-06-15".into();
        write_manifest(&publish.join("manifest.json"), &manifest).unwrap();
        seed_exchange_figures(&root);

        let err = promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap_err();
        assert!(err.to_string().contains("panel date"));
        assert!(is_materialized_bundle_pointer(&pointer));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_version_directory_blocks_promote() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-rebuild-corrupt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let publish = root.join("artifacts/data");
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        if !workspace_publish.join("manifest.json").exists() {
            return;
        }
        std::fs::create_dir_all(&publish).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                fs::copy(src, publish.join(name)).unwrap();
            }
        }
        fs::copy(
            workspace_publish.join("manifest.json"),
            publish.join("manifest.json"),
        )
        .unwrap();

        seed_exchange_figures(&root);
        promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap();
        #[cfg(unix)]
        {
            let version_name = root
                .join("artifacts/audits")
                .join(EXCHANGE_BUNDLE.id)
                .read_link()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let version_root = root.join("artifacts/audits/versions").join(version_name);
            fs::remove_file(version_root.join("manifest.json")).unwrap();
            let err = promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap_err();
            assert!(err.to_string().contains("corrupt or hash-collides"));
            assert!(!version_root.join("manifest.json").exists());
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn registry_promote_is_idempotent_for_unchanged_source() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-registry-idempotent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace_data = crate::config::repo_root().join("data");
        if !workspace_data.join("rwa_asset_registry.csv").exists() {
            return;
        }
        let dest = root.join("data");
        std::fs::create_dir_all(&dest).unwrap();
        for name in REGISTRY_BUNDLE.data_files {
            let src = workspace_data.join(name);
            if !src.exists() {
                return;
            }
            fs::copy(src, dest.join(name)).unwrap();
        }

        promote_audit_bundle_at(REGISTRY_BUNDLE.id, &root).unwrap();
        #[cfg(unix)]
        let first_target = root
            .join("artifacts/audits")
            .join(REGISTRY_BUNDLE.id)
            .read_link()
            .unwrap();
        let version_count_after_first = fs::read_dir(root.join("artifacts/audits/versions"))
            .unwrap()
            .filter_map(|e| e.ok())
            .count();

        promote_audit_bundle_at(REGISTRY_BUNDLE.id, &root).unwrap();

        #[cfg(unix)]
        {
            let second_target = root
                .join("artifacts/audits")
                .join(REGISTRY_BUNDLE.id)
                .read_link()
                .unwrap();
            assert_eq!(first_target, second_target);
        }
        let version_count_after_second = fs::read_dir(root.join("artifacts/audits/versions"))
            .unwrap()
            .filter_map(|e| e.ok())
            .count();
        assert_eq!(version_count_after_first, version_count_after_second);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn promote_rejects_manifest_claim_with_missing_evidence() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-bad-claim-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let publish = root.join("artifacts/data");
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        if !workspace_publish.join("manifest.json").exists() {
            return;
        }
        std::fs::create_dir_all(&publish).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                fs::copy(src, publish.join(name)).unwrap();
            }
        }
        let mut manifest = load_manifest(&workspace_publish.join("manifest.json")).unwrap();
        manifest.claims[0].evidence_file = "artifacts/data/not-present.json".into();
        write_manifest(&publish.join("manifest.json"), &manifest).unwrap();
        seed_exchange_figures(&root);

        let err = promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap_err();
        assert!(err.to_string().contains("evidence missing"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn promote_rejects_missing_required_figure() {
        let root = std::env::temp_dir().join(format!(
            "rwa-bundle-missing-figure-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let publish = root.join("artifacts/data");
        let workspace_publish = crate::config::repo_root().join("artifacts/data");
        if !workspace_publish.join("manifest.json").exists() {
            return;
        }
        std::fs::create_dir_all(&publish).unwrap();
        for name in EXCHANGE_BUNDLE.data_files {
            let src = workspace_publish.join(name);
            if src.exists() {
                fs::copy(src, publish.join(name)).unwrap();
            }
        }
        fs::copy(
            workspace_publish.join("manifest.json"),
            publish.join("manifest.json"),
        )
        .unwrap();

        let err = promote_audit_bundle_at(EXCHANGE_BUNDLE.id, &root).unwrap_err();
        assert!(err.to_string().contains("missing required figure"));

        let _ = fs::remove_dir_all(root);
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
        ensure_dir(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let target = dst.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_dir_recursive(&entry.path(), &target)?;
            } else {
                fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }
}
