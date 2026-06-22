use std::env;
use std::process;

use anyhow::Result;

use crate::audit::{
    parse_run_mode, resolve_module, run_module, AuditContext, ExchangeRunArgs, RunExtra, RunMode,
};
use crate::core::bundle::{list_bundle_specs, promote_audit_bundle, EXCHANGE_BUNDLE};
use crate::exchange::config::exchange_live_staging_dir;
use crate::exchange::{freeze_exchange_evidence, ExchangeFreezeOptions};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Runtime(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct RunCommand {
    pub module: String,
    pub mode: RunMode,
    pub assets: Option<String>,
    pub refresh_rwa: bool,
    pub promote: bool,
    pub tx_hashes: Vec<String>,
}

#[derive(Debug)]
pub struct FreezeCommand {
    pub action: FreezeAction,
}

#[derive(Debug)]
pub enum FreezeAction {
    List,
    Promote { audit_id: String },
    Exchange { live: bool, refresh_rwa: bool },
}

pub fn run_cli(argv: impl IntoIterator<Item = String>) -> Result<(), CliError> {
    let mut args = argv.into_iter();
    let _prog = args.next();
    let top = args
        .next()
        .ok_or_else(|| usage_error("missing subcommand"))?;

    match top.as_str() {
        "run" => {
            let cmd = parse_run_command(&mut args)?;
            validate_run_flags(&cmd)?;
            dispatch_run(cmd)
        }
        "freeze" => {
            let cmd = parse_freeze_command(&mut args)?;
            dispatch_freeze(cmd)
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(usage_error(format!("unknown command: {other}"))),
    }
}

fn dispatch_run(cmd: RunCommand) -> Result<(), CliError> {
    let mut ctx = AuditContext::new()?;
    if let Some(path) = cmd.assets {
        ctx = ctx.with_registry_assets_path(path.into());
    }

    let extra = RunExtra {
        tx_hashes: cmd.tx_hashes,
        exchange: ExchangeRunArgs {
            refresh_rwa_xyz: cmd.refresh_rwa,
        },
        promote_bundle: cmd.promote,
    };

    let bundle = run_module(&cmd.module, &ctx, cmd.mode, &extra)?;
    println!("{}", bundle.summary);
    for path in &bundle.files_written {
        if path.exists() {
            println!("  {}", path.display());
        }
    }
    Ok(())
}

fn dispatch_freeze(cmd: FreezeCommand) -> Result<(), CliError> {
    match cmd.action {
        FreezeAction::List => {
            for spec in list_bundle_specs() {
                println!("{}", spec.id);
            }
        }
        FreezeAction::Promote { audit_id } => {
            let dest = promote_audit_bundle(&audit_id)?;
            println!("Promoted bundle → {}", dest.display());
        }
        FreezeAction::Exchange { live, refresh_rwa } => {
            // Hold the exchange lock for both live and non-live so concurrent
            // `freeze exchange` invocations never interleave writes to either
            // data/exchange-live/ or the non-live staging directory.
            let exchange_lock =
                crate::core::bundle::acquire_collect_promote_lock(EXCHANGE_BUNDLE.id)
                    .map_err(CliError::Runtime)?;
            freeze_exchange_evidence(ExchangeFreezeOptions {
                live_apis: live,
                refresh_rwa_xyz: refresh_rwa,
                panel_date: None,
            })?;
            if live {
                drop(exchange_lock);
                println!(
                    "Live exchange evidence written to {}; bundle not promoted",
                    exchange_live_staging_dir().display()
                );
            } else {
                let bundle = crate::core::publish::resolve_publish_bundle(EXCHANGE_BUNDLE.id)
                    .map_err(|e| CliError::Runtime(anyhow::anyhow!("{e}")))?;
                let dest = crate::core::bundle::promote_publish_bundle_after_collect(
                    bundle,
                    exchange_lock,
                )?;
                println!("Exchange bundle → {}", dest.display());
            }
        }
    }
    Ok(())
}

pub fn parse_run_command(args: &mut impl Iterator<Item = String>) -> Result<RunCommand, CliError> {
    let module = args
        .next()
        .ok_or_else(|| usage_error("run requires <module>"))?;

    let mut mode = RunMode::Live;
    let mut assets = None;
    let mut refresh_rwa = false;
    let mut promote = false;
    let mut tx_hashes = Vec::new();

    if module == "exchange" {
        mode = RunMode::Frozen {
            snapshot_date: None,
        };
    }

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => {
                let raw = args
                    .next()
                    .ok_or_else(|| usage_error("--mode requires live|frozen"))?;
                let snapshot_date = match &mode {
                    RunMode::Frozen { snapshot_date } => snapshot_date.clone(),
                    RunMode::Live => None,
                };
                mode =
                    parse_run_mode(&raw, snapshot_date).map_err(|e| usage_error(e.to_string()))?;
            }
            "--publish-date" => {
                let date = args
                    .next()
                    .ok_or_else(|| usage_error("--publish-date requires YYYY-MM-DD"))?;
                mode = RunMode::Frozen {
                    snapshot_date: Some(date),
                };
            }
            "--assets" => {
                assets = Some(
                    args.next()
                        .ok_or_else(|| usage_error("--assets requires path"))?,
                );
            }
            "--refresh-rwa" => refresh_rwa = true,
            "--promote" => promote = true,
            a if a.starts_with("0x") => tx_hashes.push(a.to_string()),
            other => return Err(usage_error(format!("unknown run flag or arg: {other}"))),
        }
    }

    if module == "exchange" && !promote && !mode.is_live() {
        promote = true;
    }

    Ok(RunCommand {
        module,
        mode,
        assets,
        refresh_rwa,
        promote,
        tx_hashes,
    })
}

fn validate_run_flags(cmd: &RunCommand) -> Result<(), CliError> {
    let module = cmd.module.as_str();

    if cmd.refresh_rwa && module != "exchange" {
        return Err(usage_error(format!(
            "--refresh-rwa is only valid for exchange (module: {module})"
        )));
    }
    if cmd.promote {
        let m = resolve_module(module).map_err(|e| usage_error(e.to_string()))?;
        if m.publish_bundle().is_none() {
            return Err(usage_error(format!(
                "--promote is not supported for {module} (no publish bundle)"
            )));
        }
    }
    if cmd.assets.is_some() && !matches!(module, "registry" | "article1") {
        return Err(usage_error(format!(
            "--assets is only valid for registry and article1 (module: {module})"
        )));
    }
    if let RunMode::Frozen {
        snapshot_date: Some(_),
    } = &cmd.mode
    {
        if module != "exchange" {
            return Err(usage_error("--publish-date is only valid for exchange"));
        }
    }
    if !cmd.tx_hashes.is_empty() && module != "flow-tx" {
        return Err(usage_error(format!(
            "transaction hashes are only valid for flow-tx (module: {module})"
        )));
    }
    if matches!(
        module,
        "registry" | "activity" | "article1" | "flow-panel" | "flow-quotes" | "flow-tx"
    ) && !cmd.mode.is_live()
    {
        return Err(usage_error(format!("{module} only supports --mode live")));
    }
    if cmd.promote && cmd.mode.is_live() && module == "exchange" {
        return Err(usage_error(
            "cannot use --promote with exchange --mode live; live output goes to data/exchange-live/ and must not update the publish bundle",
        ));
    }

    Ok(())
}

pub fn parse_freeze_command(
    args: &mut impl Iterator<Item = String>,
) -> Result<FreezeCommand, CliError> {
    let action = args
        .next()
        .ok_or_else(|| usage_error("freeze requires list|promote|exchange"))?;

    match action.as_str() {
        "list" => Ok(FreezeCommand {
            action: FreezeAction::List,
        }),
        "promote" => {
            let audit_id = args
                .next()
                .ok_or_else(|| usage_error("freeze promote requires <audit-id>"))?;
            Ok(FreezeCommand {
                action: FreezeAction::Promote { audit_id },
            })
        }
        "exchange" => {
            let mut live = false;
            let mut refresh_rwa = false;
            for arg in args {
                match arg.as_str() {
                    "--live" => live = true,
                    "--refresh-rwa" => refresh_rwa = true,
                    other => return Err(usage_error(format!("unknown freeze flag: {other}"))),
                }
            }
            Ok(FreezeCommand {
                action: FreezeAction::Exchange { live, refresh_rwa },
            })
        }
        other => Err(usage_error(format!("unknown freeze action: {other}"))),
    }
}

fn usage_error(msg: impl Into<String>) -> CliError {
    CliError::Usage(msg.into())
}

pub fn print_help() {
    eprintln!(
        "rwa-audit — RWA evidence collection and publish bundles

USAGE:
    rwa-audit run <module> [options] [0x<tx_hash>...]
    rwa-audit freeze <list|promote|exchange> [options]

RUN MODULES:
    registry      Article 1 contract registry + transfer metrics (--mode live, no promote)
    activity      Article 1 daily activity timeseries (--mode live, no promote)
    article1      Article 1 full evidence: registry + activity, then promote (--mode live)
    flow-panel    Article 2 DEX pool panel (--mode live)
    flow-quotes   Article 2 ParaSwap quotes (--mode live)
    flow-tx       Article 2 tx reconstruction (requires 0x hashes)
    exchange      Article 3 xStocks freeze (default --mode frozen, auto --promote in frozen mode)

RUN OPTIONS:
    --mode live|frozen
    --publish-date YYYY-MM-DD   (offline freeze: must match publish fixture 2026-06-12)
    --assets PATH               (registry YAML; article1 and registry only)
    --refresh-rwa               (exchange: refresh RWA.xyz seed)
    --promote                   (article1, exchange: promote publish bundle after collection)

FREEZE:
    rwa-audit freeze list
    rwa-audit freeze promote <audit-id>
    rwa-audit freeze exchange [--live] [--refresh-rwa]   (promotes only when not --live)

Legacy binaries (rwa-collect, rwa-freeze, ...) remain available.
"
    );
}

pub fn main_entry() {
    if let Err(e) = run_cli(env::args()) {
        match e {
            CliError::Usage(msg) => {
                eprintln!("{msg}");
                print_help();
                process::exit(2);
            }
            CliError::Runtime(err) => {
                eprintln!("Error: {err:#}");
                process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::list_run_targets;

    fn args(s: &str) -> Vec<String> {
        std::iter::once("rwa-audit".to_string())
            .chain(s.split_whitespace().map(str::to_string))
            .collect()
    }

    #[test]
    fn parse_run_exchange_live_does_not_auto_promote() {
        let mut iter = args("run exchange --mode live").into_iter();
        iter.next();
        iter.next();
        let cmd = parse_run_command(&mut iter).unwrap();
        assert!(cmd.mode.is_live());
        assert!(!cmd.promote);
    }

    #[test]
    fn parse_run_exchange_live_promote_is_rejected() {
        let err = validate_run_flags(
            &parse_run_command(
                &mut args("run exchange --mode live --promote")
                    .into_iter()
                    .skip(2),
            )
            .unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn registry_rejects_refresh_rwa() {
        let err = validate_run_flags(
            &parse_run_command(&mut args("run registry --refresh-rwa").into_iter().skip(2))
                .unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn activity_rejects_assets_flag() {
        let err = validate_run_flags(
            &parse_run_command(
                &mut args("run activity --assets config/assets/registry_v1.yaml")
                    .into_iter()
                    .skip(2),
            )
            .unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_run_exchange_frozen_auto_promotes() {
        let mut iter = args("run exchange").into_iter();
        iter.next();
        iter.next();
        let cmd = parse_run_command(&mut iter).unwrap();
        assert_eq!(cmd.module, "exchange");
        assert!(!cmd.mode.is_live());
        assert!(cmd.promote);
    }

    #[test]
    fn parse_run_flow_tx_collects_hashes() {
        let mut iter = args("run flow-tx 0xabc 0xdef").into_iter();
        iter.next();
        iter.next();
        let cmd = parse_run_command(&mut iter).unwrap();
        assert_eq!(cmd.tx_hashes.len(), 2);
    }

    #[test]
    fn parse_freeze_promote() {
        let mut iter = args("freeze promote article3-xstocks-2026-06-12").into_iter();
        iter.next();
        iter.next();
        let cmd = parse_freeze_command(&mut iter).unwrap();
        assert!(matches!(cmd.action, FreezeAction::Promote { .. }));
    }

    #[test]
    fn unknown_command_is_usage_error() {
        let err = run_cli(args("bogus")).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn list_run_targets_matches_help_modules() {
        let targets = list_run_targets();
        assert_eq!(targets.len(), 7);
    }

    #[test]
    fn activity_rejects_promote_no_bundle() {
        let err = validate_run_flags(
            &parse_run_command(&mut args("run activity --promote").into_iter().skip(2)).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn article1_accepts_promote() {
        validate_run_flags(
            &parse_run_command(&mut args("run article1 --promote").into_iter().skip(2)).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn registry_rejects_promote_no_bundle() {
        let err = validate_run_flags(
            &parse_run_command(&mut args("run registry --promote").into_iter().skip(2)).unwrap(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn run_cli_help_succeeds() {
        assert!(run_cli(args("help")).is_ok());
    }

    #[test]
    fn run_cli_missing_subcommand_is_usage_error() {
        // args("") produces ["rwa-audit", ""] — two elements; the second is the subcommand
        // We need truly no subcommand: build the vec manually.
        let argv = vec!["rwa-audit".to_string()];
        let err = run_cli(argv).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_run_mode_flag_overrides_default() {
        let mut iter = args("run registry --mode live").into_iter().skip(2);
        let cmd = parse_run_command(&mut iter).unwrap();
        assert!(cmd.mode.is_live());
    }

    #[test]
    fn parse_run_publish_date_sets_frozen_mode() {
        let mut iter = args("run exchange --publish-date 2026-06-12")
            .into_iter()
            .skip(2);
        let cmd = parse_run_command(&mut iter).unwrap();
        assert!(!cmd.mode.is_live());
        assert!(matches!(
            &cmd.mode,
            RunMode::Frozen {
                snapshot_date: Some(_)
            }
        ));
    }

    #[test]
    fn parse_run_assets_flag() {
        let mut iter = args("run registry --assets config/foo.yaml")
            .into_iter()
            .skip(2);
        let cmd = parse_run_command(&mut iter).unwrap();
        assert!(cmd.assets.is_some());
        assert_eq!(cmd.assets.unwrap(), "config/foo.yaml");
    }

    #[test]
    fn parse_run_unknown_flag_is_usage_error() {
        let mut iter = args("run registry --bogus").into_iter().skip(2);
        let err = parse_run_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_tx_hashes_on_non_flow_tx_is_error() {
        let cmd = RunCommand {
            module: "registry".to_string(),
            mode: RunMode::Live,
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec!["0xabc".to_string()],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_publish_date_on_non_exchange_is_error() {
        let cmd = RunCommand {
            module: "registry".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: Some("2026-06-12".to_string()),
            },
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec![],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_non_exchange_frozen_mode_is_error() {
        let mut iter = args("run flow-panel --mode frozen").into_iter().skip(2);
        let cmd = parse_run_command(&mut iter).unwrap();
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_freeze_list_parses() {
        let mut iter = args("freeze list").into_iter().skip(2);
        let cmd = parse_freeze_command(&mut iter).unwrap();
        assert!(matches!(cmd.action, FreezeAction::List));
    }

    #[test]
    fn parse_freeze_exchange_with_flags() {
        let mut iter = args("freeze exchange --live --refresh-rwa")
            .into_iter()
            .skip(2);
        let cmd = parse_freeze_command(&mut iter).unwrap();
        assert!(matches!(
            cmd.action,
            FreezeAction::Exchange {
                live: true,
                refresh_rwa: true
            }
        ));
    }

    #[test]
    fn parse_freeze_unknown_action_is_error() {
        let mut iter = args("freeze bogus").into_iter().skip(2);
        let err = parse_freeze_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_freeze_exchange_unknown_flag_is_error() {
        let mut iter = args("freeze exchange --bogus").into_iter().skip(2);
        let err = parse_freeze_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_freeze_promote_requires_audit_id() {
        let mut iter = args("freeze promote").into_iter().skip(2);
        let err = parse_freeze_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn run_cli_freeze_list_succeeds() {
        assert!(run_cli(args("freeze list")).is_ok());
    }

    #[test]
    fn activity_rejects_frozen_mode() {
        let cmd = RunCommand {
            module: "activity".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec![],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_run_mode_flag_missing_value_is_error() {
        let mut iter = args("run registry --mode").into_iter().skip(2);
        let err = parse_run_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn run_cli_help_aliases() {
        assert!(run_cli(args("--help")).is_ok());
        assert!(run_cli(args("-h")).is_ok());
    }

    #[test]
    fn parse_run_missing_module_is_error() {
        let err = parse_run_command(&mut std::iter::empty()).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
        assert!(err.to_string().contains("module"));
    }

    #[test]
    fn parse_freeze_missing_action_is_error() {
        let err = parse_freeze_command(&mut std::iter::empty()).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_exchange_frozen_promote_ok() {
        let cmd = RunCommand {
            module: "exchange".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: false,
            promote: true,
            tx_hashes: vec![],
        };
        assert!(validate_run_flags(&cmd).is_ok());
    }

    #[test]
    fn validate_run_assets_on_flow_panel_is_error() {
        let cmd = RunCommand {
            module: "flow-panel".to_string(),
            mode: RunMode::Live,
            assets: Some("some/path.yaml".into()),
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec![],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_run_refresh_rwa_on_exchange_is_ok() {
        let cmd = RunCommand {
            module: "exchange".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: true,
            promote: true,
            tx_hashes: vec![],
        };
        assert!(validate_run_flags(&cmd).is_ok());
    }

    #[test]
    fn parse_run_publish_date_missing_value_is_error() {
        let mut iter = args("run exchange --publish-date").into_iter().skip(2);
        let err = parse_run_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_run_assets_missing_value_is_error() {
        let mut iter = args("run registry --assets").into_iter().skip(2);
        let err = parse_run_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_run_mode_invalid_value_is_error() {
        let mut iter = args("run registry --mode hybrid").into_iter().skip(2);
        let err = parse_run_command(&mut iter).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_article1_with_frozen_is_error() {
        let cmd = RunCommand {
            module: "article1".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec![],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
        assert!(err.to_string().contains("live"));
    }

    #[test]
    fn validate_flow_quotes_with_frozen_is_error() {
        let cmd = RunCommand {
            module: "flow-quotes".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec![],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_flow_tx_with_frozen_is_error() {
        let cmd = RunCommand {
            module: "flow-tx".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec!["0xabc".to_string()],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn validate_registry_with_frozen_is_error() {
        let cmd = RunCommand {
            module: "registry".to_string(),
            mode: RunMode::Frozen {
                snapshot_date: None,
            },
            assets: None,
            refresh_rwa: false,
            promote: false,
            tx_hashes: vec![],
        };
        let err = validate_run_flags(&cmd).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }

    #[test]
    fn parse_freeze_exchange_default_no_flags() {
        let mut iter = args("freeze exchange").into_iter().skip(2);
        let cmd = parse_freeze_command(&mut iter).unwrap();
        assert!(matches!(
            cmd.action,
            FreezeAction::Exchange {
                live: false,
                refresh_rwa: false
            }
        ));
    }

    #[test]
    fn cli_error_usage_display() {
        let e = CliError::Usage("test message".into());
        assert_eq!(e.to_string(), "test message");
    }

    #[test]
    fn parse_run_refresh_rwa_flag() {
        let mut iter = args("run exchange --refresh-rwa").into_iter().skip(2);
        let cmd = parse_run_command(&mut iter).unwrap();
        assert!(cmd.refresh_rwa);
    }

    #[test]
    fn parse_run_promote_flag() {
        let mut iter = args("run article1 --promote").into_iter().skip(2);
        let cmd = parse_run_command(&mut iter).unwrap();
        assert!(cmd.promote);
    }

    #[test]
    fn run_cli_unknown_top_level_is_usage_error() {
        let err = run_cli(args("unknown-cmd")).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
        assert!(err.to_string().contains("unknown command"));
    }
}
