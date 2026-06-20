fn main() -> anyhow::Result<()> {
    eprintln!("warning: rwa-collect is deprecated; use 'rwa-audit run registry' instead");
    let ctx = rwa_audit::AuditContext::new()?;
    rwa_audit::run_module(
        "registry",
        &ctx,
        rwa_audit::RunMode::Live,
        &rwa_audit::audit::RunExtra::default(),
    )?;
    Ok(())
}
