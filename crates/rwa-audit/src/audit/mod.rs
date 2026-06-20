//! Unified audit module contract and runners.

mod cli;
mod modules;

pub use cli::{main_entry, parse_freeze_command, parse_run_command, CliError, FreezeAction};
pub use modules::{exchange_run_mode, parse_run_mode, resolve_module, ExchangeRunArgs};

use std::path::PathBuf;

use anyhow::Result;

use crate::asset_config::{default_activity_path, default_registry_path};
use crate::core::manifest::AuditMethod;
use crate::sources::SourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunMode {
    Live,
    Frozen { snapshot_date: Option<String> },
}

impl RunMode {
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Live)
    }
}

#[derive(Debug, Clone)]
pub struct AuditContext {
    pub registry_assets_path: PathBuf,
    pub activity_assets_path: PathBuf,
}

impl AuditContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            registry_assets_path: default_registry_path(),
            activity_assets_path: default_activity_path(),
        })
    }

    pub fn with_registry_assets_path(mut self, path: PathBuf) -> Self {
        self.registry_assets_path = path;
        self
    }
}

#[derive(Debug, Clone)]
pub struct EvidenceBundle {
    pub module: String,
    pub method: AuditMethod,
    pub mode: RunMode,
    pub files_written: Vec<PathBuf>,
    pub summary: String,
    /// Carrying panel_date back from the module so the runner can pass it to
    /// `PublishBundle::validate_before_promote` without re-deriving it.
    pub panel_date: Option<String>,
}

pub trait AuditModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn method(&self) -> AuditMethod;
    fn required_sources(&self) -> Vec<SourceId>;
    fn run(&self, ctx: &AuditContext, mode: RunMode, extra: &RunExtra) -> Result<EvidenceBundle>;

    /// When set, frozen runs may promote flat evidence into `artifacts/audits/{id}/`.
    fn publish_bundle(&self) -> Option<&'static dyn crate::core::publish::PublishBundle> {
        None
    }

    /// Lock key for the shared data directory this module writes to.
    /// Modules that write to the same directory must return the same key so the runner
    /// serializes them. `run registry`, `run activity`, and `run article1` all share
    /// `REGISTRY_BUNDLE.id` because they all write to `data/`.
    fn data_write_lock_id(&self) -> Option<&'static str> {
        None
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunExtra {
    pub tx_hashes: Vec<String>,
    pub exchange: ExchangeRunArgs,
    /// Promote the module's `publish_bundle()` into `artifacts/audits/` after a successful run.
    pub promote_bundle: bool,
}

pub fn run_module(
    name: &str,
    ctx: &AuditContext,
    mode: RunMode,
    extra: &RunExtra,
) -> Result<EvidenceBundle> {
    let module = resolve_module(name)?;

    // Acquire the exclusive lock BEFORE collection. Two cases need it:
    // 1. Any module declaring data_write_lock_id — serializes concurrent writers to
    //    the same shared directory (e.g. `run registry` and `run article1 --promote`).
    // 2. Any module with publish_bundle when --promote — additionally covers promotion.
    // Both cases share the same lock when the ids match (article1 family), so a
    // single lock covers the full registry/activity/article1 data directory.
    let effective_lock_id = module.data_write_lock_id().or_else(|| {
        if extra.promote_bundle {
            module.publish_bundle().map(|pb| pb.id())
        } else {
            None
        }
    });
    let collect_promote_lock = effective_lock_id
        .map(crate::core::bundle::acquire_collect_promote_lock)
        .transpose()?;

    let mut evidence = module.run(ctx, mode, extra)?;
    if extra.promote_bundle {
        if let Some(pb) = module.publish_bundle() {
            let from_live = evidence.mode.is_live();
            pb.validate_before_promote(evidence.panel_date.as_deref().unwrap_or(""), from_live)?;
            let lock = collect_promote_lock
                .expect("lock acquired above when promote_bundle && publish_bundle is Some");
            let bundle_path = crate::core::bundle::promote_publish_bundle_after_collect(pb, lock)?;
            evidence
                .summary
                .push_str(&format!("; bundle → {}", bundle_path.display()));
            evidence
                .files_written
                .push(bundle_path.join("manifest.json"));
        }
    }
    Ok(evidence)
}

pub fn list_run_targets() -> Vec<&'static str> {
    modules::all_module_names()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_mode_live_flag() {
        assert!(RunMode::Live.is_live());
        assert!(!RunMode::Frozen {
            snapshot_date: None
        }
        .is_live());
    }

    #[test]
    fn audit_context_default_paths_exist() {
        let ctx = AuditContext::new().unwrap();
        assert!(ctx.registry_assets_path.ends_with("registry_v1.yaml"));
        assert!(ctx.activity_assets_path.ends_with("activity_v1.yaml"));
    }

    #[test]
    fn list_run_targets_includes_core_modules() {
        let names = list_run_targets();
        for m in ["registry", "activity", "article1", "flow-panel", "exchange"] {
            assert!(names.contains(&m), "missing module {m}");
        }
    }
}
