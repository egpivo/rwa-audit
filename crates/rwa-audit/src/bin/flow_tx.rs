fn main() -> anyhow::Result<()> {
    let hashes: Vec<String> = std::env::args().skip(1).collect();
    let ctx = rwa_audit::AuditContext::new()?;
    rwa_audit::run_module(
        "flow-tx",
        &ctx,
        rwa_audit::RunMode::Live,
        &rwa_audit::audit::RunExtra {
            tx_hashes: hashes,
            ..Default::default()
        },
    )?;
    Ok(())
}
