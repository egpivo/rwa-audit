use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde_json::Value;

pub struct HttpTransport {
    inner: Client,
}

/// Outcome of a single HTTP GET attempt (after in-transport retries for 429 only).
#[derive(Debug)]
pub enum HttpGetResult<T> {
    Ok(T),
    RateLimited,
    /// A 4xx response whose JSON body was preserved for adapter-level semantic handling.
    /// Only emitted by [`HttpTransport::http_get_json_with_status`].
    ClientError {
        status: u16,
        body: Value,
    },
}

impl HttpTransport {
    pub fn new() -> Result<Self> {
        let inner = Client::builder()
            .user_agent("rwa-audit/0.1")
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { inner })
    }

    pub fn http_get(
        &self,
        url: &str,
        params: &[(&str, &str)],
        retries: u32,
    ) -> Result<HttpGetResult<Value>> {
        self.http_get_with_headers(url, params, &[], retries)
    }

    pub fn http_get_with_headers(
        &self,
        url: &str,
        params: &[(&str, &str)],
        headers: &[(&str, &str)],
        retries: u32,
    ) -> Result<HttpGetResult<Value>> {
        self.http_get_inner(url, params, headers, retries, |resp| {
            resp.json().context("parse JSON response")
        })
    }

    pub fn http_get_text_with_headers(
        &self,
        url: &str,
        params: &[(&str, &str)],
        headers: &[(&str, &str)],
        retries: u32,
    ) -> Result<HttpGetResult<String>> {
        self.http_get_inner(url, params, headers, retries, |resp| {
            resp.text().context("read text response")
        })
    }

    fn http_get_inner<T, F>(
        &self,
        url: &str,
        params: &[(&str, &str)],
        headers: &[(&str, &str)],
        retries: u32,
        parse: F,
    ) -> Result<HttpGetResult<T>>
    where
        F: Fn(reqwest::blocking::Response) -> Result<T>,
    {
        let attempts = retries.max(1);
        let mut rate_limited = false;
        for attempt in 0..attempts {
            let mut req = self.inner.get(url);
            if !params.is_empty() {
                req = req.query(params);
            }
            for (name, value) in headers {
                req = req.header(*name, *value);
            }
            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        rate_limited = true;
                        let wait = 3 * 2u64.pow(attempt);
                        eprintln!("    Rate limited, sleeping {wait}s...");
                        thread::sleep(Duration::from_secs(wait));
                        continue;
                    }
                    if !status.is_success() {
                        anyhow::bail!("HTTP {status} for {url}");
                    }
                    return Ok(HttpGetResult::Ok(parse(resp)?));
                }
                Err(e) => {
                    eprintln!("    Request error ({}/{}): {e}", attempt + 1, attempts);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        if rate_limited {
            return Ok(HttpGetResult::RateLimited);
        }
        anyhow::bail!("HTTP GET failed for {url} after {attempts} attempts")
    }

    /// Like [`http_get_with_headers`] but returns [`HttpGetResult::ClientError`] for 4xx
    /// instead of bailing. Callers that can assign semantic meaning to 4xx responses (e.g.
    /// "no route found") should use this; all other callers use the standard methods.
    pub fn http_get_json_with_status(
        &self,
        url: &str,
        params: &[(&str, &str)],
        headers: &[(&str, &str)],
        retries: u32,
    ) -> Result<HttpGetResult<Value>> {
        let attempts = retries.max(1);
        let mut rate_limited = false;
        for attempt in 0..attempts {
            let mut req = self.inner.get(url);
            if !params.is_empty() {
                req = req.query(params);
            }
            for (name, value) in headers {
                req = req.header(*name, *value);
            }
            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        rate_limited = true;
                        let wait = 3 * 2u64.pow(attempt);
                        eprintln!("    Rate limited, sleeping {wait}s...");
                        thread::sleep(Duration::from_secs(wait));
                        continue;
                    }
                    if status.is_client_error() {
                        let body: Value = resp.json().unwrap_or(Value::Null);
                        return Ok(HttpGetResult::ClientError {
                            status: status.as_u16(),
                            body,
                        });
                    }
                    if !status.is_success() {
                        anyhow::bail!("HTTP {status} for {url}");
                    }
                    let body: Value = resp.json().context("parse JSON response")?;
                    return Ok(HttpGetResult::Ok(body));
                }
                Err(e) => {
                    eprintln!("    Request error ({}/{}): {e}", attempt + 1, attempts);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        if rate_limited {
            return Ok(HttpGetResult::RateLimited);
        }
        anyhow::bail!("HTTP GET failed for {url} after {attempts} attempts")
    }

    pub fn rpc_post(&self, rpc_url: &str, payload: &Value, retries: u32) -> Result<Option<Value>> {
        let attempts = retries.max(1);
        for attempt in 0..attempts {
            match self
                .inner
                .post(rpc_url)
                .header("Content-Type", "application/json")
                .json(payload)
                .send()
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        thread::sleep(Duration::from_secs(2u64.pow(attempt) * 2));
                        continue;
                    }
                    if !status.is_success() {
                        anyhow::bail!("RPC HTTP {status} for {rpc_url}");
                    }
                    return Ok(Some(resp.json()?));
                }
                Err(e) => {
                    eprintln!("    RPC error ({}/{}): {e}", attempt + 1, attempts);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::test_support::MockHttpServer;

    #[test]
    fn transport_builds_client() {
        HttpTransport::new().unwrap();
    }

    #[test]
    fn http_get_sends_query_and_headers_and_parses_json() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"ok":true}"#);
        let result = HttpTransport::new()
            .unwrap()
            .http_get_with_headers(
                &server.url,
                &[("q", "a b")],
                &[("x-audit-test", "present")],
                1,
            )
            .unwrap();

        match result {
            HttpGetResult::Ok(body) => assert_eq!(body["ok"], true),
            other => panic!("unexpected result: {other:?}"),
        }
        let request = server.request().to_ascii_lowercase();
        assert!(request.starts_with("get /?q=a+b ") || request.starts_with("get /?q=a%20b "));
        assert!(request.contains("x-audit-test: present"));
    }

    #[test]
    fn http_get_parses_text() {
        let server = MockHttpServer::spawn("200 OK", "text/plain", "audit evidence");
        let result = HttpTransport::new()
            .unwrap()
            .http_get_text_with_headers(&server.url, &[], &[], 1)
            .unwrap();

        match result {
            HttpGetResult::Ok(body) => assert_eq!(body, "audit evidence"),
            other => panic!("unexpected result: {other:?}"),
        }
        server.request();
    }

    #[test]
    fn http_get_reports_server_error() {
        let server = MockHttpServer::spawn("500 Internal Server Error", "application/json", "{}");
        let error = HttpTransport::new()
            .unwrap()
            .http_get(&server.url, &[], 1)
            .unwrap_err();

        assert!(error.to_string().contains("500"));
        server.request();
    }

    #[test]
    fn json_with_status_preserves_client_error_body() {
        let server = MockHttpServer::spawn(
            "400 Bad Request",
            "application/json",
            r#"{"error":"no route"}"#,
        );
        let result = HttpTransport::new()
            .unwrap()
            .http_get_json_with_status(&server.url, &[], &[], 1)
            .unwrap();

        match result {
            HttpGetResult::ClientError { status, body } => {
                assert_eq!(status, 400);
                assert_eq!(body["error"], "no route");
            }
            other => panic!("unexpected result: {other:?}"),
        }
        server.request();
    }

    #[test]
    fn rpc_post_sends_json_and_parses_response() {
        let server = MockHttpServer::spawn(
            "200 OK",
            "application/json",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x2a"}"#,
        );
        let body = HttpTransport::new()
            .unwrap()
            .rpc_post(
                &server.url,
                &serde_json::json!({"jsonrpc":"2.0","id":1,"method":"eth_blockNumber"}),
                1,
            )
            .unwrap()
            .unwrap();

        assert_eq!(body["result"], "0x2a");
        let request = server.request().to_ascii_lowercase();
        assert!(request.starts_with("post / "));
        assert!(request.contains("content-type: application/json"));
    }

    #[test]
    fn rpc_post_reports_server_error() {
        let server = MockHttpServer::spawn("503 Service Unavailable", "application/json", "{}");
        let error = HttpTransport::new()
            .unwrap()
            .rpc_post(&server.url, &serde_json::json!({}), 1)
            .unwrap_err();

        assert!(error.to_string().contains("503"));
        server.request();
    }

    #[test]
    fn http_get_with_empty_params_works() {
        let server = MockHttpServer::spawn("200 OK", "application/json", r#"{"empty":true}"#);
        let result = HttpTransport::new()
            .unwrap()
            .http_get(&server.url, &[], 1)
            .unwrap();
        assert!(matches!(result, HttpGetResult::Ok(_)));
        server.request();
    }

    #[test]
    fn json_with_status_5xx_bails() {
        let server =
            MockHttpServer::spawn("500 Internal Server Error", "application/json", r#"{}"#);
        let err = HttpTransport::new()
            .unwrap()
            .http_get_json_with_status(&server.url, &[], &[], 1)
            .unwrap_err();
        assert!(err.to_string().contains("500"));
        server.request();
    }
}
