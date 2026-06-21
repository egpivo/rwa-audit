use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Url;
use serde_json::Value;

use super::adapter::SourceAdapter;
use super::cache::ResponseCache;
use super::context::SourceContext;
use super::transport::HttpGetResult;
use super::types::{Provenance, SourceId, SourceResponse};

fn query_param_refs(query: &[(String, String)]) -> Vec<(&str, &str)> {
    query
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect()
}

fn is_sensitive_query_param(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "apikey" | "api_key" | "key" | "token" | "access_token"
    )
}

fn redact_query_for_provenance(query: &[(String, String)]) -> Vec<(String, String)> {
    query
        .iter()
        .map(|(k, v)| {
            if is_sensitive_query_param(k) {
                (k.clone(), "***".into())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect()
}

fn base_url_without_query(url: &str) -> String {
    let no_fragment = url.split_once('#').map(|(b, _)| b).unwrap_or(url);
    no_fragment
        .split_once('?')
        .map(|(b, _)| b)
        .unwrap_or(no_fragment)
        .to_string()
}

/// Merge query params embedded in `url` with explicit `query` (explicit wins on duplicate keys).
pub fn merged_query_params(url: &str, extra: &[(String, String)]) -> Vec<(String, String)> {
    let mut merged: Vec<(String, String)> = Vec::new();
    if let Ok(parsed) = Url::parse(url) {
        for (k, v) in parsed.query_pairs() {
            merged.push((k.into_owned(), v.into_owned()));
        }
    }
    for (k, v) in extra {
        if let Some(slot) = merged.iter_mut().find(|(mk, _)| mk == k) {
            slot.1 = v.clone();
        } else {
            merged.push((k.clone(), v.clone()));
        }
    }
    merged
}

pub fn prepare_http_get(url: &str, query: &[(String, String)]) -> (String, Vec<(String, String)>) {
    let base = base_url_without_query(url);
    let merged = merged_query_params(url, query);
    (base, merged)
}

/// Percent-encoded request URL matching what reqwest sends on the wire.
pub fn canonical_request_url(url: &str, query: &[(String, String)]) -> String {
    let (base, merged) = prepare_http_get(url, query);
    if merged.is_empty() {
        return base;
    }
    let pairs = query_param_refs(&merged);
    Url::parse_with_params(&base, &pairs)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| {
            let qs: String = merged
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&");
            format!("{base}?{qs}")
        })
}

/// Provenance-safe URL: canonical encoding with secrets redacted.
pub fn provenance_request_url(url: &str, query: &[(String, String)]) -> String {
    let (base, merged) = prepare_http_get(url, query);
    let redacted = redact_query_for_provenance(&merged);
    canonical_request_url(&base, &redacted)
}

fn cache_key_for_http(url: &str, query: &[(String, String)]) -> String {
    let (base, merged) = prepare_http_get(url, query);
    ResponseCache::key_from_url(&base, &query_param_refs(&merged))
}

/// Shared HTTP GET with cache, rate limit, and provenance.
pub fn http_get_cached(
    adapter: &dyn SourceAdapter,
    ctx: &SourceContext,
    url: &str,
    query: &[(String, String)],
    extra_headers: &[(&str, &str)],
) -> Result<SourceResponse> {
    let id = adapter.id();
    let (base, merged) = prepare_http_get(url, query);
    let provenance_url = provenance_request_url(url, query);
    let key = cache_key_for_http(url, query);
    if let Some(body) = ctx.cache().get(id, &key) {
        return Ok(SourceResponse {
            provenance: Provenance::new(id, &provenance_url, false),
            body,
        });
    }

    ctx.rate_limit_sleep(id);

    let mut headers: Vec<(&str, &str)> = Vec::new();
    if let Some(profile) = ctx.profile(id) {
        for (k, v) in &profile.default_headers {
            headers.push((k.as_str(), v.as_str()));
        }
    }
    headers.extend(extra_headers);

    let params = query_param_refs(&merged);
    let body = match ctx
        .transport()
        .http_get_with_headers(&base, &params, &headers, 3)?
    {
        HttpGetResult::Ok(body) => body,
        HttpGetResult::RateLimited => {
            anyhow::bail!("HTTP GET rate limited for {provenance_url}");
        }
        HttpGetResult::ClientError { status, .. } => {
            anyhow::bail!("HTTP {status} for {provenance_url}");
        }
    };
    ctx.cache().put(id, &key, &body)?;
    Ok(SourceResponse {
        provenance: Provenance::new(id, &provenance_url, true)
            .with_sha256(ResponseCache::body_sha256(&body)),
        body,
    })
}

/// Like [`http_get_cached`] but treats 4xx responses as data rather than errors.
///
/// The 4xx JSON body is cached and returned in `SourceResponse.body` so the calling
/// adapter can inspect it and convert it to a structured "no result" data row.
/// Use only for adapters (e.g. ParaSwap) where a 4xx carries semantic content.
pub fn http_get_cached_or_error(
    adapter: &dyn SourceAdapter,
    ctx: &SourceContext,
    url: &str,
    query: &[(String, String)],
    extra_headers: &[(&str, &str)],
) -> Result<SourceResponse> {
    let id = adapter.id();
    let (base, merged) = prepare_http_get(url, query);
    let provenance_url = provenance_request_url(url, query);
    let key = cache_key_for_http(url, query);
    if let Some(body) = ctx.cache().get(id, &key) {
        return Ok(SourceResponse {
            provenance: Provenance::new(id, &provenance_url, false),
            body,
        });
    }

    ctx.rate_limit_sleep(id);

    let mut headers: Vec<(&str, &str)> = Vec::new();
    if let Some(profile) = ctx.profile(id) {
        for (k, v) in &profile.default_headers {
            headers.push((k.as_str(), v.as_str()));
        }
    }
    headers.extend(extra_headers);

    let params = query_param_refs(&merged);
    let body = match ctx
        .transport()
        .http_get_json_with_status(&base, &params, &headers, 3)?
    {
        HttpGetResult::Ok(body) => body,
        HttpGetResult::RateLimited => {
            anyhow::bail!("HTTP GET rate limited for {provenance_url}");
        }
        HttpGetResult::ClientError { body, .. } => body,
    };
    ctx.cache().put(id, &key, &body)?;
    Ok(SourceResponse {
        provenance: Provenance::new(id, &provenance_url, true)
            .with_sha256(ResponseCache::body_sha256(&body)),
        body,
    })
}

/// GeckoTerminal uses longer backoff on 429 only.
pub fn http_get_gecko(
    adapter: &dyn SourceAdapter,
    ctx: &SourceContext,
    url: &str,
    query: &[(String, String)],
) -> Result<SourceResponse> {
    let id = adapter.id();
    let (base, merged) = prepare_http_get(url, query);
    let provenance_url = provenance_request_url(url, query);
    let key = cache_key_for_http(url, query);
    if let Some(body) = ctx.cache().get(id, &key) {
        return Ok(SourceResponse {
            provenance: Provenance::new(id, &provenance_url, false),
            body,
        });
    }

    let headers = [("accept", "application/json")];
    let params = query_param_refs(&merged);
    for attempt in 0..5u32 {
        ctx.rate_limit_sleep(id);
        match ctx
            .transport()
            .http_get_with_headers(&base, &params, &headers, 1)?
        {
            HttpGetResult::Ok(body) => {
                ctx.cache().put(id, &key, &body)?;
                return Ok(SourceResponse {
                    provenance: Provenance::new(id, &provenance_url, true)
                        .with_sha256(ResponseCache::body_sha256(&body)),
                    body,
                });
            }
            HttpGetResult::RateLimited => {
                let wait = 5 * 2u64.pow(attempt);
                eprintln!("    GeckoTerminal rate limited, sleeping {wait}s...");
                thread::sleep(Duration::from_secs(wait));
            }
            HttpGetResult::ClientError { status, .. } => {
                anyhow::bail!("HTTP {status} for {provenance_url}");
            }
        }
    }
    anyhow::bail!("GeckoTerminal rate limit exceeded for {provenance_url}")
}

/// HTML/text GET with cache (body stored as JSON string).
pub fn http_get_text_cached(
    id: SourceId,
    ctx: &SourceContext,
    url: &str,
    query: &[(String, String)],
) -> Result<SourceResponse> {
    ctx.require_profile(id)?;
    let (base, merged) = prepare_http_get(url, query);
    let provenance_url = provenance_request_url(url, query);
    let key = cache_key_for_http(url, query);
    if let Some(body) = ctx.cache().get(id, &key) {
        let text = body
            .as_str()
            .context("cached text response was not a string")?
            .to_string();
        return Ok(SourceResponse {
            provenance: Provenance::new(id, &provenance_url, false),
            body: Value::String(text),
        });
    }

    ctx.rate_limit_sleep(id);
    let params = query_param_refs(&merged);
    let text = match ctx
        .transport()
        .http_get_text_with_headers(&base, &params, &[], 3)?
    {
        HttpGetResult::Ok(text) => text,
        HttpGetResult::RateLimited => {
            anyhow::bail!("HTTP GET (text) rate limited for {provenance_url}");
        }
        HttpGetResult::ClientError { status, .. } => {
            anyhow::bail!("HTTP {status} for {provenance_url}");
        }
    };
    let body = Value::String(text);
    ctx.cache().put(id, &key, &body)?;
    Ok(SourceResponse {
        provenance: Provenance::new(id, &provenance_url, true)
            .with_sha256(ResponseCache::body_sha256(&body)),
        body,
    })
}

pub fn response_text(resp: &SourceResponse) -> Result<String> {
    resp.body
        .as_str()
        .map(str::to_string)
        .context("expected text response body")
}

pub fn rpc_fetch_cached(
    adapter: &dyn SourceAdapter,
    ctx: &SourceContext,
    url: &str,
    method: &str,
    params: &Value,
    payload: &Value,
) -> Result<SourceResponse> {
    let id = adapter.id();
    let key = ResponseCache::key_from_rpc(url, method, params);
    if let Some(body) = ctx.cache().get_rpc(method, params, id, &key) {
        return Ok(SourceResponse {
            provenance: Provenance::new(id, url, false),
            body,
        });
    }

    ctx.rate_limit_sleep(id);

    let body = ctx
        .transport()
        .rpc_post(url, payload, 3)?
        .with_context(|| format!("RPC {method} failed for {url}"))?;
    ctx.cache().put_rpc(method, params, id, &key, &body)?;
    Ok(SourceResponse {
        provenance: Provenance::new(id, url, true).with_sha256(ResponseCache::body_sha256(&body)),
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use crate::sources::adapters::{
        CoinGeckoAdapter, GeckoTerminalAdapter, ParaSwapAdapter, PublicNodeRpcAdapter,
    };
    use crate::sources::profile::{SourceKind, SourceProfile};
    use crate::sources::registry::SourceRegistry;
    use crate::sources::test_support::MockHttpServer;

    fn temp_cache(name: &str) -> (ResponseCache, PathBuf) {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("rwa-fetch-{name}-{suffix}"));
        (ResponseCache::new(root.clone()), root)
    }

    fn context_for(id: SourceId, base_url: &str, cache: ResponseCache) -> SourceContext {
        let rpc_endpoints = if id == SourceId::PublicNodeRpc {
            HashMap::from([("ethereum".to_string(), base_url.to_string())])
        } else {
            HashMap::new()
        };
        let profile = SourceProfile {
            id,
            kind: if id == SourceId::PublicNodeRpc {
                SourceKind::Rpc
            } else {
                SourceKind::Http
            },
            base_url: Some(base_url.to_string()),
            rpc_endpoints,
            base_path: None,
            env_keys: vec![],
            rate_limit_ms: 0,
            default_headers: HashMap::from([("x-default".into(), "configured".into())]),
        };
        SourceContext::with_registry(SourceRegistry::from_profiles(HashMap::from([(
            id, profile,
        )])))
        .unwrap()
        .with_cache(cache)
    }

    #[test]
    fn canonical_request_url_percent_encodes_values() {
        let url = canonical_request_url(
            "https://api.example.com/v1",
            &[("q".into(), "a b".into()), ("sym".into(), "A&B".into())],
        );
        assert!(url.contains("q=a+b") || url.contains("q=a%20b"));
        assert!(url.contains("A%26B") || url.contains("A&B"));
    }

    #[test]
    fn canonical_request_url_merges_embedded_query() {
        let url = canonical_request_url(
            "https://api.example.com/v1?existing=1",
            &[("added".into(), "2".into())],
        );
        assert!(url.contains("existing=1"));
        assert!(url.contains("added=2"));
    }

    #[test]
    fn provenance_url_redacts_api_key() {
        let url = provenance_request_url(
            "https://api.ethplorer.io/getTokenInfo/0xabc",
            &[("apiKey".into(), "paid-secret-key".into())],
        );
        assert!(url.contains("apiKey="));
        assert!(!url.contains("paid-secret-key"));
        assert!(url.contains("***"));
    }

    #[test]
    fn cache_key_uses_unredacted_query() {
        let query = vec![("apiKey".into(), "secret".into())];
        let key_a = cache_key_for_http("https://api.example.com", &query);
        let key_b = ResponseCache::key_from_url("https://api.example.com", &[("apiKey", "secret")]);
        assert_eq!(key_a, key_b);
        let redacted = provenance_request_url("https://api.example.com", &query);
        assert!(!redacted.contains("secret"));
    }

    #[test]
    fn canonical_request_url_without_query_is_unchanged() {
        assert_eq!(
            canonical_request_url("https://api.example.com/v1", &[]),
            "https://api.example.com/v1"
        );
    }

    #[test]
    fn cached_get_fetches_once_and_records_provenance() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"usd":42}"#);
        let (cache, root) = temp_cache("json");
        let ctx = context_for(SourceId::CoinGecko, &server.url, cache);
        let query = vec![
            ("asset".into(), "buidl".into()),
            ("api_key".into(), "secret".into()),
        ];

        let live = http_get_cached(
            &CoinGeckoAdapter,
            &ctx,
            &server.url,
            &query,
            &[("x-extra", "yes")],
        )
        .unwrap();
        let cached = http_get_cached(&CoinGeckoAdapter, &ctx, &server.url, &query, &[]).unwrap();

        assert_eq!(live.body["usd"], 42);
        assert!(live.provenance.live);
        assert!(live.provenance.response_sha256.is_some());
        assert!(!live.provenance.request_url.contains("secret"));
        assert_eq!(cached.body, live.body);
        assert!(!cached.provenance.live);
        let request = server.request().to_ascii_lowercase();
        assert!(request.contains("x-default: configured"));
        assert!(request.contains("x-extra: yes"));
        assert!(request.contains("api_key=secret"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cached_get_rejects_server_errors() {
        let server = MockHttpServer::spawn("500 Internal Server Error", "application/json", "{}");
        let ctx = context_for(SourceId::CoinGecko, &server.url, ResponseCache::disabled());
        let error = http_get_cached(&CoinGeckoAdapter, &ctx, &server.url, &[], &[]).unwrap_err();

        assert!(error.to_string().contains("500"));
        server.request();
    }

    #[test]
    fn semantic_client_error_is_returned_and_cached() {
        let server = MockHttpServer::spawn(
            "400 Bad Request",
            "application/json",
            r#"{"error":"no route"}"#,
        );
        let (cache, root) = temp_cache("client-error");
        let ctx = context_for(SourceId::ParaSwap, &server.url, cache);

        let live = http_get_cached_or_error(&ParaSwapAdapter, &ctx, &server.url, &[], &[]).unwrap();
        let cached =
            http_get_cached_or_error(&ParaSwapAdapter, &ctx, &server.url, &[], &[]).unwrap();

        assert_eq!(live.body["error"], "no route");
        assert!(live.provenance.live);
        assert!(!cached.provenance.live);
        server.request();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn gecko_get_fetches_and_caches_success() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"data":[]}"#);
        let (cache, root) = temp_cache("gecko");
        let ctx = context_for(SourceId::GeckoTerminal, &server.url, cache);

        let live = http_get_gecko(&GeckoTerminalAdapter, &ctx, &server.url, &[]).unwrap();
        let cached = http_get_gecko(&GeckoTerminalAdapter, &ctx, &server.url, &[]).unwrap();

        assert_eq!(live.body, json!({"data": []}));
        assert!(live.provenance.live);
        assert!(!cached.provenance.live);
        let request = server.request().to_ascii_lowercase();
        assert!(request.contains("accept: application/json"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn text_get_fetches_caches_and_extracts_text() {
        let server = MockHttpServer::spawn("200 OK", "text/html", "<h1>evidence</h1>");
        let (cache, root) = temp_cache("text");
        let ctx = context_for(SourceId::RwaXyz, &server.url, cache);

        let live = http_get_text_cached(SourceId::RwaXyz, &ctx, &server.url, &[]).unwrap();
        let cached = http_get_text_cached(SourceId::RwaXyz, &ctx, &server.url, &[]).unwrap();

        assert_eq!(response_text(&live).unwrap(), "<h1>evidence</h1>");
        assert_eq!(cached.body, live.body);
        assert!(!cached.provenance.live);
        assert!(response_text(&SourceResponse {
            provenance: live.provenance.clone(),
            body: json!({"not": "text"}),
        })
        .is_err());
        server.request();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn merged_query_params_merges_url_and_extra_params() {
        let params = merged_query_params(
            "https://api.example.com?a=1&b=2",
            &[("c".into(), "3".into())],
        );
        assert!(params.iter().any(|(k, v)| k == "a" && v == "1"));
        assert!(params.iter().any(|(k, v)| k == "b" && v == "2"));
        assert!(params.iter().any(|(k, v)| k == "c" && v == "3"));
    }

    #[test]
    fn merged_query_params_explicit_overrides_url_param() {
        let params = merged_query_params(
            "https://api.example.com?a=1",
            &[("a".into(), "override".into())],
        );
        let a_vals: Vec<_> = params.iter().filter(|(k, _)| k == "a").collect();
        assert_eq!(a_vals.len(), 1);
        assert_eq!(a_vals[0].1, "override");
    }

    #[test]
    fn merged_query_params_empty_inputs_returns_empty() {
        let params = merged_query_params("https://api.example.com", &[]);
        assert!(params.is_empty());
    }

    #[test]
    fn gecko_server_error_propagates_as_err() {
        let server =
            MockHttpServer::spawn("500 Internal Server Error", "application/json", r#"{}"#);
        let ctx = context_for(SourceId::GeckoTerminal, &server.url, ResponseCache::disabled());
        let err =
            http_get_gecko(&GeckoTerminalAdapter, &ctx, &server.url, &[]).unwrap_err();
        assert!(err.to_string().contains("500"));
        server.request();
    }

    #[test]
    fn text_get_requires_profile_to_be_registered() {
        let ctx =
            SourceContext::with_registry(SourceRegistry::from_profiles(HashMap::new()))
                .unwrap()
                .with_cache(ResponseCache::disabled());
        let err =
            http_get_text_cached(SourceId::RwaXyz, &ctx, "https://example.com", &[]).unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn rpc_fetches_once_and_uses_cache() {
        let server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x2a"}"#,
        );
        let (cache, root) = temp_cache("rpc");
        let ctx = context_for(SourceId::PublicNodeRpc, &server.url, cache);
        let params = json!([]);
        let payload = json!({"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":params});

        let live = rpc_fetch_cached(
            &PublicNodeRpcAdapter,
            &ctx,
            &server.url,
            "eth_blockNumber",
            &params,
            &payload,
        )
        .unwrap();
        let cached = rpc_fetch_cached(
            &PublicNodeRpcAdapter,
            &ctx,
            &server.url,
            "eth_blockNumber",
            &params,
            &payload,
        )
        .unwrap();

        assert_eq!(live.body["result"], "0x2a");
        assert!(live.provenance.live);
        assert!(!cached.provenance.live);
        server.request();
        let _ = fs::remove_dir_all(root);
    }
}
