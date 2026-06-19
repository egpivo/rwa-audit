use std::env;
use std::process;

use anyhow::Result;

use crate::audit::{parse_run_mode, run_module, AuditContext, ExchangeRunArgs, RunExtra, RunMode};
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

pub struct RunCommand {
    pub module: String,
    pub mode: RunMode,
    pub assets: Option<String>,
    pub refresh_rwa: bool,
    pub promote: bool,
    pub tx_hashes: Vec<String>,
}

pub struct FreezeCommand {
    pub action: FreezeAction,
}

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
            promote_bundle: cmd.promote,
        },
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
            freeze_exchange_evidence(ExchangeFreezeOptions {
                live_apis: live,
                refresh_rwa_xyz: refresh_rwa,
                panel_date: None,
            })?;
            if live {
                println!(
                    "Live exchange evidence written to {}; bundle not promoted",
                    exchange_live_staging_dir().display()
                );
            } else {
                let dest = promote_audit_bundle(EXCHANGE_BUNDLE.id)?;
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
    if cmd.promote && module != "exchange" {
        return Err(usage_error(format!(
            "--promote is only valid for exchange (module: {module})"
        )));
    }
    if cmd.assets.is_some() && module != "registry" {
        return Err(usage_error(format!(
            "--assets is only valid for registry (module: {module})"
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
        "registry" | "activity" | "flow-panel" | "flow-quotes" | "flow-tx"
    ) && !cmd.mode.is_live()
    {
        return Err(usage_error(format!("{module} only supports --mode live")));
    }
    if module == "exchange" && cmd.mode.is_live() && cmd.promote {
        return Err(usage_error(
            "cannot use --promote with exchange --mode live; live output is written to data/exchange-live/ and must not update the publish bundle",
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
    registry      Article 1 contract registry + transfer metrics (--mode live)
    activity      Article 1 daily activity (--mode live)
    flow-panel    Article 2 DEX pool panel (--mode live)
    flow-quotes   Article 2 ParaSwap quotes (--mode live)
    flow-tx       Article 2 tx reconstruction (requires 0x hashes)
    exchange      Article 3 xStocks freeze (default --mode frozen, auto --promote in frozen mode)

RUN OPTIONS:
    --mode live|frozen
    --publish-date YYYY-MM-DD   (offline freeze: must match publish fixture 2026-06-12)
    --assets PATH               (registry YAML, default config/assets/registry_v1.yaml)
    --refresh-rwa               (exchange: refresh RWA.xyz seed)
    --promote                   (exchange frozen only: promote publish bundle; not allowed with --mode live)

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
        assert_eq!(targets.len(), 6);
    }
}
