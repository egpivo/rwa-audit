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

    #[test]
    fn transport_builds_client() {
        HttpTransport::new().unwrap();
    }
}
