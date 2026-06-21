use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::activity::collect_activity_timeseries;
use crate::collect::collect_all;
use crate::config::data_dir;
use crate::core::manifest::AuditMethod;
use crate::core::publish::{
    ExchangePublishBundle, RegistryPublishBundle, EXCHANGE_BUNDLE, REGISTRY_BUNDLE,
};
use crate::exchange::config::exchange_output_dir;
use crate::exchange::{freeze_exchange_evidence, ExchangeFreezeOptions};
use crate::flow::panel::collect_flow_panel;
use crate::flow::paraswap::collect_paraswap_quotes;
use crate::flow::tx_recon::reconstruct_case_studies;
use crate::sources::SourceId;

use super::{AuditContext, AuditModule, EvidenceBundle, RunExtra, RunMode};

pub struct RegistryModule;
pub struct ActivityModule;
pub struct Article1Module;
pub struct FlowPanelModule;
pub struct FlowQuotesModule;
pub struct FlowTxModule;
pub struct ExchangeModule;

const MODULE_NAMES: &[&str] = &[
    "registry",
    "activity",
    "article1",
    "flow-panel",
    "flow-quotes",
    "flow-tx",
    "exchange",
];

pub fn all_module_names() -> Vec<&'static str> {
    MODULE_NAMES.to_vec()
}

pub fn resolve_module(name: &str) -> Result<&'static dyn AuditModule> {
    match name {
        "registry" => Ok(&RegistryModule),
        "activity" => Ok(&ActivityModule),
        "article1" => Ok(&Article1Module),
        "flow-panel" => Ok(&FlowPanelModule),
        "flow-quotes" => Ok(&FlowQuotesModule),
        "flow-tx" => Ok(&FlowTxModule),
        "exchange" => Ok(&ExchangeModule),
        other => bail!("unknown audit module: {other}"),
    }
}

pub fn parse_run_mode(raw: &str, snapshot_date: Option<String>) -> Result<RunMode> {
    match raw {
        "live" => Ok(RunMode::Live),
        "frozen" => Ok(RunMode::Frozen { snapshot_date }),
        other => bail!("unknown run mode: {other} (expected live|frozen)"),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExchangeRunArgs {
    pub refresh_rwa_xyz: bool,
}

pub fn exchange_run_mode(mode: &RunMode) -> ExchangeFreezeOptions {
    let panel_date = match mode {
        RunMode::Frozen { snapshot_date } => snapshot_date.clone(),
        RunMode::Live => None,
    };
    ExchangeFreezeOptions {
        refresh_rwa_xyz: false,
        live_apis: mode.is_live(),
        panel_date,
    }
}

impl AuditModule for RegistryModule {
    fn name(&self) -> &'static str {
        "registry"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::Registry
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![
            SourceId::PublicNodeRpc,
            SourceId::CoinGecko,
            SourceId::Ethplorer,
        ]
    }

    fn data_write_lock_id(&self) -> Option<&'static str> {
        Some(REGISTRY_BUNDLE.id)
    }

    fn run(&self, ctx: &AuditContext, mode: RunMode, _extra: &RunExtra) -> Result<EvidenceBundle> {
        if !mode.is_live() {
            bail!("registry module only supports --mode live (on-chain collection)");
        }
        collect_all(&ctx.registry_assets_path)?;
        let out = data_dir();
        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: registry_output_files(&out),
            summary: format!("Registry metrics written to {}", out.display()),
            panel_date: None,
        })
    }
}

impl AuditModule for ActivityModule {
    fn name(&self) -> &'static str {
        "activity"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::Activity
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![SourceId::PublicNodeRpc]
    }

    fn data_write_lock_id(&self) -> Option<&'static str> {
        Some(REGISTRY_BUNDLE.id)
    }

    fn run(&self, ctx: &AuditContext, mode: RunMode, _extra: &RunExtra) -> Result<EvidenceBundle> {
        if !mode.is_live() {
            bail!("activity module only supports --mode live");
        }
        let observation_date = collect_activity_timeseries(&ctx.activity_assets_path)?;
        let path = data_dir().join("rwa_activity_daily_30d.csv");
        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: vec![path.clone()],
            summary: format!(
                "Activity series written to {} (as_of {})",
                path.display(),
                observation_date
            ),
            panel_date: Some(observation_date),
        })
    }
}

impl AuditModule for Article1Module {
    fn name(&self) -> &'static str {
        "article1"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::Registry
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![
            SourceId::PublicNodeRpc,
            SourceId::CoinGecko,
            SourceId::Ethplorer,
        ]
    }

    fn publish_bundle(&self) -> Option<&'static dyn crate::core::publish::PublishBundle> {
        static BUNDLE: RegistryPublishBundle = RegistryPublishBundle;
        Some(&BUNDLE)
    }

    fn data_write_lock_id(&self) -> Option<&'static str> {
        Some(REGISTRY_BUNDLE.id)
    }

    fn run(&self, ctx: &AuditContext, mode: RunMode, _extra: &RunExtra) -> Result<EvidenceBundle> {
        if !mode.is_live() {
            bail!("article1 only supports --mode live (on-chain collection)");
        }
        collect_all(&ctx.registry_assets_path)?;
        let observation_date = collect_activity_timeseries(&ctx.activity_assets_path)?;
        let out = data_dir();
        let mut files = registry_output_files(&out);
        files.push(out.join("rwa_activity_daily_30d.csv"));
        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: files,
            summary: format!(
                "Article 1 evidence (registry + activity) written to {} (as_of {})",
                out.display(),
                observation_date
            ),
            panel_date: Some(observation_date),
        })
    }
}

impl AuditModule for FlowPanelModule {
    fn name(&self) -> &'static str {
        "flow-panel"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::FlowSurface
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![SourceId::GeckoTerminal, SourceId::YahooFinance]
    }

    fn run(&self, _ctx: &AuditContext, mode: RunMode, _extra: &RunExtra) -> Result<EvidenceBundle> {
        if !mode.is_live() {
            bail!("flow-panel only supports --mode live (GeckoTerminal + Yahoo)");
        }
        collect_flow_panel()?;
        let out = crate::flow::config::flow_data_dir();
        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: vec![out.join("panel_daily.csv"), out.join("panel_summary.json")],
            summary: format!("Flow panel written to {}", out.display()),
            panel_date: None,
        })
    }
}

impl AuditModule for FlowQuotesModule {
    fn name(&self) -> &'static str {
        "flow-quotes"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::FlowSurface
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![SourceId::ParaSwap]
    }

    fn run(&self, _ctx: &AuditContext, mode: RunMode, _extra: &RunExtra) -> Result<EvidenceBundle> {
        if !mode.is_live() {
            bail!("flow-quotes only supports --mode live");
        }
        collect_paraswap_quotes()?;
        let out = crate::flow::config::flow_data_dir();
        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: vec![out.join("paraswap_quotes.csv")],
            summary: format!("ParaSwap quotes written to {}", out.display()),
            panel_date: None,
        })
    }
}

impl AuditModule for FlowTxModule {
    fn name(&self) -> &'static str {
        "flow-tx"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::FlowSurface
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![SourceId::PublicNodeRpc]
    }

    fn run(&self, _ctx: &AuditContext, mode: RunMode, extra: &RunExtra) -> Result<EvidenceBundle> {
        if !mode.is_live() {
            bail!("flow-tx only supports --mode live");
        }
        if extra.tx_hashes.is_empty() {
            bail!("flow-tx requires at least one transaction hash argument");
        }
        reconstruct_case_studies(&extra.tx_hashes)?;
        let path = crate::flow::config::flow_data_dir().join("tx_reconstructions.json");
        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: vec![path.clone()],
            summary: format!(
                "Reconstructed {} tx case(s) → {}",
                extra.tx_hashes.len(),
                path.display()
            ),
            panel_date: None,
        })
    }
}

impl AuditModule for ExchangeModule {
    fn name(&self) -> &'static str {
        "exchange"
    }

    fn method(&self) -> AuditMethod {
        AuditMethod::ExchangeSurface
    }

    fn required_sources(&self) -> Vec<SourceId> {
        vec![
            SourceId::ManualImport,
            SourceId::GeckoTerminal,
            SourceId::Jupiter,
        ]
    }

    fn publish_bundle(&self) -> Option<&'static dyn crate::core::publish::PublishBundle> {
        static BUNDLE: ExchangePublishBundle = ExchangePublishBundle;
        Some(&BUNDLE)
    }

    fn data_write_lock_id(&self) -> Option<&'static str> {
        Some(EXCHANGE_BUNDLE.id)
    }

    fn run(&self, ctx: &AuditContext, mode: RunMode, extra: &RunExtra) -> Result<EvidenceBundle> {
        let mut opts = exchange_run_mode(&mode);
        opts.refresh_rwa_xyz = extra.exchange.refresh_rwa_xyz;
        let _ = ctx;
        freeze_exchange_evidence(opts.clone())?;

        let out_dir = exchange_output_dir(opts.live_apis);
        let panel_date = crate::exchange::config::resolve_panel_date(
            opts.live_apis,
            opts.panel_date.as_deref(),
        )?;

        let summary = if opts.live_apis {
            format!(
                "Live exchange evidence written to {} (publish staging untouched)",
                out_dir.display()
            )
        } else {
            format!("Exchange evidence frozen to {}", out_dir.display())
        };

        Ok(EvidenceBundle {
            module: self.name().into(),
            method: self.method(),
            mode,
            files_written: exchange_output_files(&out_dir),
            summary,
            panel_date: Some(panel_date),
        })
    }
}

fn registry_output_files(dir: &Path) -> Vec<PathBuf> {
    [
        "rwa_asset_registry.csv",
        "rwa_transfer_metrics.csv",
        "rwa_holder_metrics.csv",
        "rwa_mint_burn_metrics.csv",
        "rwa_data_quality_notes.md",
    ]
    .iter()
    .map(|f| dir.join(f))
    .collect()
}

fn exchange_output_files(dir: &Path) -> Vec<PathBuf> {
    vec![
        dir.join("manifest.json"),
        dir.join("depth_vs_volume_panel_publish.csv"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_run_mode_variants() {
        assert!(matches!(
            parse_run_mode("live", None).unwrap(),
            RunMode::Live
        ));
        assert!(matches!(
            parse_run_mode("frozen", Some("2026-06-12".into())).unwrap(),
            RunMode::Frozen { .. }
        ));
        assert!(parse_run_mode("hybrid", None).is_err());
    }

    #[test]
    fn resolve_all_modules() {
        for name in all_module_names() {
            let m = resolve_module(name).unwrap();
            assert_eq!(m.name(), name);
        }
    }

    #[test]
    fn registry_requires_live_mode() {
        let ctx = AuditContext::new().unwrap();
        let err = RegistryModule
            .run(
                &ctx,
                RunMode::Frozen {
                    snapshot_date: Some("2026-06-12".into()),
                },
                &RunExtra::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn exchange_frozen_passes_panel_date() {
        let opts = exchange_run_mode(&RunMode::Frozen {
            snapshot_date: Some("2026-06-15".into()),
        });
        assert!(!opts.live_apis);
        assert_eq!(opts.panel_date.as_deref(), Some("2026-06-15"));
    }

    #[test]
    fn exchange_frozen_uses_offline_apis() {
        let opts = exchange_run_mode(&RunMode::Frozen {
            snapshot_date: Some("2026-06-12".into()),
        });
        assert!(!opts.live_apis);
    }

    #[test]
    fn flow_tx_requires_hashes() {
        let ctx = AuditContext::new().unwrap();
        let err = FlowTxModule
            .run(&ctx, RunMode::Live, &RunExtra::default())
            .unwrap_err();
        assert!(err.to_string().contains("hash"));
    }

    #[test]
    fn module_source_requirements_are_non_empty() {
        for name in all_module_names() {
            let m = resolve_module(name).unwrap();
            assert!(
                !m.required_sources().is_empty(),
                "{name} should declare sources"
            );
        }
    }

    #[test]
    fn default_registry_path_matches_config() {
        let _ = crate::asset_config::default_registry_path();
        assert!(resolve_module("registry").is_ok());
    }

    #[test]
    fn resolve_module_unknown_name_bails() {
        assert!(resolve_module("nonexistent").is_err());
    }

    #[test]
    fn data_write_lock_ids_are_set_where_expected() {
        assert!(RegistryModule.data_write_lock_id().is_some());
        assert!(ActivityModule.data_write_lock_id().is_some());
        assert!(Article1Module.data_write_lock_id().is_some());
        assert!(ExchangeModule.data_write_lock_id().is_some());
        assert!(FlowPanelModule.data_write_lock_id().is_none());
        assert!(FlowQuotesModule.data_write_lock_id().is_none());
        assert!(FlowTxModule.data_write_lock_id().is_none());
    }

    #[test]
    fn exchange_module_has_publish_bundle() {
        assert!(ExchangeModule.publish_bundle().is_some());
    }

    #[test]
    fn article1_module_has_publish_bundle() {
        assert!(Article1Module.publish_bundle().is_some());
    }

    #[test]
    fn exchange_run_mode_live_sets_live_apis() {
        let opts = exchange_run_mode(&RunMode::Live);
        assert!(opts.live_apis);
        assert!(opts.panel_date.is_none());
    }

    #[test]
    fn flow_panel_rejects_frozen_mode() {
        let ctx = AuditContext::new().unwrap();
        let err = FlowPanelModule
            .run(
                &ctx,
                RunMode::Frozen {
                    snapshot_date: None,
                },
                &RunExtra::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn flow_quotes_rejects_frozen_mode() {
        let ctx = AuditContext::new().unwrap();
        let err = FlowQuotesModule
            .run(
                &ctx,
                RunMode::Frozen {
                    snapshot_date: None,
                },
                &RunExtra::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn activity_module_rejects_frozen_mode() {
        let ctx = AuditContext::new().unwrap();
        let err = ActivityModule
            .run(
                &ctx,
                RunMode::Frozen {
                    snapshot_date: None,
                },
                &RunExtra::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn article1_module_rejects_frozen_mode() {
        let ctx = AuditContext::new().unwrap();
        let err = Article1Module
            .run(
                &ctx,
                RunMode::Frozen {
                    snapshot_date: Some("2026-06-12".into()),
                },
                &RunExtra::default(),
            )
            .unwrap_err();
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn flow_tx_module_rejects_frozen_mode() {
        let ctx = AuditContext::new().unwrap();
        let extra = RunExtra {
            tx_hashes: vec!["0xabc".into()],
            ..Default::default()
        };
        let err = FlowTxModule
            .run(
                &ctx,
                RunMode::Frozen {
                    snapshot_date: None,
                },
                &extra,
            )
            .unwrap_err();
        assert!(err.to_string().contains("live"));
    }
}
