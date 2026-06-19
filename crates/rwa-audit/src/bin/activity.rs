fn main() -> anyhow::Result<()> {
    rwa_audit::collect_activity_timeseries(&rwa_audit::asset_config::default_activity_path())
}
