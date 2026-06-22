fn main() -> anyhow::Result<()> {
    eprintln!("warning: rwa-activity is deprecated; use 'rwa-audit run activity' instead");
    let ctx = rwa_audit::AuditContext::new()?;
    rwa_audit::run_module(
        "activity",
        &ctx,
        rwa_audit::RunMode::Live,
        &rwa_audit::audit::RunExtra::default(),
    )?;
    Ok(())
}
