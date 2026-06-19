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
}

pub trait AuditModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn method(&self) -> AuditMethod;
    fn required_sources(&self) -> Vec<SourceId>;
    fn run(&self, ctx: &AuditContext, mode: RunMode, extra: &RunExtra) -> Result<EvidenceBundle>;
}

#[derive(Debug, Clone, Default)]
pub struct RunExtra {
    pub tx_hashes: Vec<String>,
    pub exchange: ExchangeRunArgs,
}

pub fn run_module(
    name: &str,
    ctx: &AuditContext,
    mode: RunMode,
    extra: &RunExtra,
) -> Result<EvidenceBundle> {
    let module = resolve_module(name)?;
    module.run(ctx, mode, extra)
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
        for m in ["registry", "activity", "flow-panel", "exchange"] {
            assert!(names.contains(&m), "missing module {m}");
        }
    }
}
