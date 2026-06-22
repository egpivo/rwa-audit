fn main() -> anyhow::Result<()> {
    let ctx = rwa_audit::AuditContext::new()?;
    rwa_audit::run_module(
        "flow-quotes",
        &ctx,
        rwa_audit::RunMode::Live,
        &rwa_audit::audit::RunExtra::default(),
    )?;
    Ok(())
}
