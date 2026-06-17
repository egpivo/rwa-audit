use rwa_audit::exchange::{freeze_exchange_evidence, ExchangeFreezeOptions};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let opts = ExchangeFreezeOptions {
        refresh_rwa_xyz: args.iter().any(|a| a == "--refresh-rwa"),
        live_apis: args.iter().any(|a| a == "--live"),
    };
    freeze_exchange_evidence(opts)
}
