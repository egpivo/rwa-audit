use anyhow::{bail, Context, Result};
use chrono::NaiveDate;
use serde::Serialize;
use serde_json::Value;

use super::super::adapter::SourceAdapter;
use super::super::context::SourceContext;
use super::super::fetch::http_get_cached;
use super::super::types::{SourceId, SourceRequest, SourceResponse};

pub struct YahooFinanceAdapter;

impl SourceAdapter for YahooFinanceAdapter {
    fn id(&self) -> SourceId {
        SourceId::YahooFinance
    }

    fn fetch(&self, ctx: &SourceContext, req: SourceRequest) -> Result<SourceResponse> {
        let SourceRequest::HttpGet { url, query } = req else {
            bail!("YahooFinanceAdapter expects HttpGet request");
        };
        http_get_cached(self, ctx, &url, &query, &[])
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GoldDaily {
    pub date: NaiveDate,
    pub close: f64,
    pub abs_return: f64,
}

impl YahooFinanceAdapter {
    pub fn fetch_gc_futures(
        &self,
        ctx: &SourceContext,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<GoldDaily>> {
        let base = ctx.http_base_url(SourceId::YahooFinance)?;
        let url = format!("{base}/v8/finance/chart/GC=F");
        let resp = self.fetch(
            ctx,
            SourceRequest::HttpGet {
                url,
                query: vec![
                    ("interval".into(), "1d".into()),
                    ("range".into(), "6mo".into()),
                ],
            },
        )?;
        Self::parse_gc_chart(&resp.body, start, end)
    }

    pub fn parse_gc_chart(
        body: &Value,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<GoldDaily>> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn date(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parses_chart_filters_window_and_computes_returns() {
        let body = json!({
            "chart": {
                "result": [{
                    "timestamp": [1772323200, 1772409600, 1772496000, 1772582400],
                    "indicators": {"quote": [{"close": [100.0, 110.0, null, 99.0]}]}
                }]
            }
        });

        let rows =
            YahooFinanceAdapter::parse_gc_chart(&body, date("2026-03-02"), date("2026-03-04"))
                .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, date("2026-03-02"));
        assert!((rows[0].abs_return - 0.1).abs() < 1e-12);
        assert_eq!(rows[1].date, date("2026-03-04"));
        assert!((rows[1].abs_return - 0.1).abs() < 1e-12);
    }

    #[test]
    fn parse_chart_rejects_missing_or_invalid_fields() {
        assert!(YahooFinanceAdapter::parse_gc_chart(
            &json!({"chart": {"result": []}}),
            date("2026-03-01"),
            date("2026-03-04")
        )
        .is_err());

        let invalid_ts = json!({
            "chart": {
                "result": [{
                    "timestamp": ["bad"],
                    "indicators": {"quote": [{"close": [100.0]}]}
                }]
            }
        });
        assert!(YahooFinanceAdapter::parse_gc_chart(
            &invalid_ts,
            date("2026-03-01"),
            date("2026-03-04")
        )
        .is_err());
    }

    #[test]
    fn adapter_rejects_rpc_requests() {
        let ctx = SourceContext::new().unwrap();
        let err = YahooFinanceAdapter
            .fetch(
                &ctx,
                SourceRequest::Rpc {
                    url: "http://example.test".into(),
                    method: "eth_blockNumber".into(),
                    params: json!([]),
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("expects HttpGet"));
    }
}
