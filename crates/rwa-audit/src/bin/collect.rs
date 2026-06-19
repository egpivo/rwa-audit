fn main() -> anyhow::Result<()> {
    rwa_audit::collect_all(&rwa_audit::asset_config::default_registry_path())
}
