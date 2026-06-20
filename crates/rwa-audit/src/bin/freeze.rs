//! Promote live or legacy flat outputs into versioned audit bundles under `artifacts/audits/`.

use anyhow::Result;

use rwa_audit::core::bundle::{list_bundle_specs, promote_audit_bundle};

fn usage() -> ! {
    eprintln!(
        "Usage:
  rwa-freeze list
  rwa-freeze promote <audit-id>
  rwa-freeze exchange [--live] [--refresh-rwa]

Examples:
  rwa-freeze promote article3-xstocks-2026-06-12
  rwa-freeze exchange --live"
    );
    std::process::exit(1);
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| usage());

    match cmd.as_str() {
        "list" => {
            for spec in list_bundle_specs() {
                println!("{}", spec.id);
            }
        }
        "promote" => {
            let audit_id = args.next().unwrap_or_else(|| usage());
            let dest = promote_audit_bundle(&audit_id)?;
            println!("Promoted bundle → {}", dest.display());
        }
        "exchange" => {
            eprintln!(
                "warning: rwa-freeze exchange is deprecated; use 'rwa-audit freeze exchange' instead"
            );
            let mut live = false;
            let mut refresh_rwa = false;
            for arg in args {
                match arg.as_str() {
                    "--live" => live = true,
                    "--refresh-rwa" => refresh_rwa = true,
                    _ => usage(),
                }
            }
            if !live {
                anyhow::bail!(
                    "rwa-freeze exchange (non-live) is no longer supported; \
                     use 'rwa-audit freeze exchange' to get correct write locking"
                );
            }
            // Live: route through run_module so the lock is applied consistently.
            let ctx = rwa_audit::AuditContext::new()?;
            let bundle = rwa_audit::run_module(
                "exchange",
                &ctx,
                rwa_audit::RunMode::Live,
                &rwa_audit::audit::RunExtra {
                    exchange: rwa_audit::audit::ExchangeRunArgs {
                        refresh_rwa_xyz: refresh_rwa,
                    },
                    ..Default::default()
                },
            )?;
            println!("{}", bundle.summary);
        }
        _ => usage(),
    }
    Ok(())
}
