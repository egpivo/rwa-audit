//! Source adapter integration tests.

use rwa_audit::sources::{CoinGeckoAdapter, ResponseCache, SourceContext};
use serde_json::json;

#[test]
fn coingecko_adapter_reads_cache_after_first_fetch() {
    let dir = std::env::temp_dir().join(format!(
        "rwa-src-cache-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let cache = ResponseCache::new(dir.clone()).with_enabled(true);
    let ctx = SourceContext::new().unwrap().with_cache(cache);

    let fixture_path =
        rwa_audit::config::repo_root().join("data/fixtures/coingecko_simple_price.json");
    if !fixture_path.exists() {
        return;
    }
    let body: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture_path).unwrap()).unwrap();
    let cg_id = "blackrock-usd-institutional-digital-liquidity-fund";
    let key = ResponseCache::key_from_url(
        "https://api.coingecko.com/api/v3/simple/price",
        &[("ids", cg_id), ("vs_currencies", "usd")],
    );
    ctx.cache()
        .put(rwa_audit::sources::SourceId::CoinGecko, &key, &body)
        .unwrap();

    let price = ctx.get_coingecko_price(cg_id).unwrap();
    assert_eq!(price, CoinGeckoAdapter::parse_price_usd(cg_id, &body));

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn manual_import_fixture_is_valid_json() {
    let path = rwa_audit::config::repo_root().join("data/fixtures/rpc_eth_blockNumber.json");
    if !path.exists() {
        return;
    }
    let body: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(body["jsonrpc"], json!("2.0"));
}
