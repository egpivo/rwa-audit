use std::thread;
use std::time::Duration;

use anyhow::Result;
use reqwest::blocking::Client;
use serde_json::Value;

pub struct HttpTransport {
    inner: Client,
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
    ) -> Result<Option<Value>> {
        for attempt in 0..retries {
            let mut req = self.inner.get(url);
            if !params.is_empty() {
                req = req.query(params);
            }
            match req.send() {
                Ok(resp) => {
                    let status = resp.status();
                    if status.as_u16() == 429 {
                        let wait = 3 * 2u64.pow(attempt);
                        eprintln!("    Rate limited, sleeping {wait}s...");
                        thread::sleep(Duration::from_secs(wait));
                        continue;
                    }
                    if !status.is_success() {
                        eprintln!("    HTTP {} for {}", status, &url[..url.len().min(80)]);
                        return Ok(None);
                    }
                    return Ok(Some(resp.json()?));
                }
                Err(e) => {
                    eprintln!("    Request error ({}/{}): {e}", attempt + 1, retries);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Ok(None)
    }

    pub fn rpc_post(&self, rpc_url: &str, payload: &Value, retries: u32) -> Result<Option<Value>> {
        for attempt in 0..retries {
            match self
                .inner
                .post(rpc_url)
                .header("Content-Type", "application/json")
                .json(payload)
                .send()
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.as_u16() == 429 {
                        thread::sleep(Duration::from_secs(2u64.pow(attempt) * 2));
                        continue;
                    }
                    if !status.is_success() {
                        eprintln!("    RPC HTTP {status}");
                        return Ok(None);
                    }
                    return Ok(Some(resp.json()?));
                }
                Err(e) => {
                    eprintln!("    RPC error ({}/{}): {e}", attempt + 1, retries);
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
