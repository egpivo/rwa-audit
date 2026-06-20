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
}
