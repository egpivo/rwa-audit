fn main() -> anyhow::Result<()> {
    eprintln!(
        "warning: rwa-exchange-freeze is deprecated; use 'rwa-audit freeze exchange' instead"
    );
    let args: Vec<String> = std::env::args().collect();
    let live = args.iter().any(|a| a == "--live");
    if !live {
        anyhow::bail!(
            "rwa-exchange-freeze (non-live) is no longer supported; \
             use 'rwa-audit freeze exchange' to get correct write locking"
        );
    }
    let refresh_rwa = args.iter().any(|a| a == "--refresh-rwa");
    let ctx = rwa_audit::AuditContext::new()?;
    rwa_audit::run_module(
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
    Ok(())
}
