//! Promote live or legacy flat outputs into versioned audit bundles under `artifacts/audits/`.

use anyhow::Result;

use rwa_audit::core::bundle::{list_bundle_specs, promote_audit_bundle, EXCHANGE_BUNDLE};
use rwa_audit::exchange::{freeze_exchange_evidence, ExchangeFreezeOptions};

fn usage() -> ! {
    eprintln!(
        "Usage:
  rwa-freeze list
  rwa-freeze promote <audit-id>
  rwa-freeze exchange [--live] [--refresh-rwa]

Examples:
  rwa-freeze promote article3-xstocks-2026-06-12
  rwa-freeze exchange"
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
            let mut opts = ExchangeFreezeOptions::default();
            for arg in args {
                match arg.as_str() {
                    "--live" => opts.live_apis = true,
                    "--refresh-rwa" => opts.refresh_rwa_xyz = true,
                    _ => usage(),
                }
            }
            let live = opts.live_apis;
            freeze_exchange_evidence(opts)?;
            if !live {
                let dest = promote_audit_bundle(EXCHANGE_BUNDLE.id)?;
                println!("Exchange bundle → {}", dest.display());
            } else {
                println!(
                    "Live exchange evidence written to {}; bundle not promoted",
                    rwa_audit::exchange::config::exchange_live_staging_dir().display()
                );
            }
        }
        _ => usage(),
    }
    Ok(())
}
