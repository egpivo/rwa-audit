fn main() -> anyhow::Result<()> {
    let hashes: Vec<String> = std::env::args().skip(1).collect();
    rwa_audit::reconstruct_case_studies(&hashes)
}
