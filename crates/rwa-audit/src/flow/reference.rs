use anyhow::{Context, Result};
use chrono::NaiveDate;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;

use crate::flow::config::YAHOO_GC_CHART;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GoldDaily {
    pub date: NaiveDate,
    pub close: f64,
    pub abs_return: f64,
}

pub fn fetch_gc_futures(start: NaiveDate, end: NaiveDate) -> Result<Vec<GoldDaily>> {
    let http = Client::builder()
        .user_agent("rwa-audit/0.1")
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp = http
        .get(YAHOO_GC_CHART)
        .query(&[("interval", "1d"), ("range", "6mo")])
        .send()
        .context("Yahoo GC=F chart")?;
    let body: Value = resp.error_for_status()?.json()?;

    let result = body
        .pointer("/chart/result/0")
        .context("yahoo chart result")?;
    let timestamps = result
        .get("timestamp")
        .and_then(|t| t.as_array())
        .context("timestamps")?;
    let closes = result
        .pointer("/indicators/quote/0/close")
        .and_then(|c| c.as_array())
        .context("closes")?;

    let mut rows = Vec::new();
    let mut prev_close: Option<f64> = None;

    for (ts, close_v) in timestamps.iter().zip(closes.iter()) {
        let close = match close_v.as_f64() {
            Some(c) => c,
            None => continue,
        };
        let ts_i = ts.as_i64().context("ts")?;
        let date = chrono::DateTime::from_timestamp(ts_i, 0)
            .map(|dt| dt.date_naive())
            .context("date")?;
        if date < start || date > end {
            prev_close = Some(close);
            continue;
        }
        let abs_return = prev_close.map(|p| ((close - p) / p).abs()).unwrap_or(0.0);
        rows.push(GoldDaily {
            date,
            close,
            abs_return,
        });
        prev_close = Some(close);
    }

    rows.sort_by_key(|r| r.date);
    Ok(rows)
}
