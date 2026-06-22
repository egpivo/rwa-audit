use std::path::Path;

use anyhow::Result;
use csv::ReaderBuilder;
use serde::Serialize;

use crate::exchange::config::bridged_export_csv;

#[derive(Debug, Clone, Serialize)]
pub struct BridgedValueSum {
    pub date: String,
    pub total_usd: f64,
    pub source_file: String,
}

pub fn sum_bridged_value_for_date(date: &str) -> Result<BridgedValueSum> {
    let path = bridged_export_csv();
    sum_bridged_value_from_csv(&path, date)
}

pub fn sum_bridged_value_from_csv(path: &Path, date: &str) -> Result<BridgedValueSum> {
    let mut rdr = ReaderBuilder::new().from_path(path)?;
    let headers = rdr.headers()?.clone();
    let skip: Vec<String> = ["Timestamp", "Date", "Measure"]
        .into_iter()
        .map(str::to_string)
        .collect();

    for result in rdr.records() {
        let r = result?;
        if r.get(1) != Some(date) {
            continue;
        }
        if r.get(2) != Some("Bridged Token Value (Dollar)") {
            continue;
        }
        let mut total = 0.0;
        for (i, h) in headers.iter().enumerate() {
            if skip.contains(&h.to_string()) {
                continue;
            }
            if let Some(cell) = r.get(i) {
                if let Ok(v) = cell.parse::<f64>() {
                    total += v;
                }
            }
        }
        return Ok(BridgedValueSum {
            date: date.into(),
            total_usd: (total * 100.0).round() / 100.0,
            source_file: path.display().to_string(),
        });
    }
    anyhow::bail!("no bridged value row for {date} in {}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridged_sum_matches_publish_target() {
        let path = bridged_export_csv();
        if !path.exists() {
            return;
        }
        let s = sum_bridged_value_from_csv(&path, "2026-06-11").unwrap();
        assert!((s.total_usd - 763_761_027.47).abs() < 1.0);
    }

    #[test]
    fn sum_bridged_value_from_csv_parses_temp_file() {
        let csv = "Timestamp,Date,Measure,TokenA,TokenB\n\
                   ts1,2026-06-11,Bridged Token Value (Dollar),500000.0,250000.0\n\
                   ts2,2026-06-11,Other Metric,99999.0,0.0\n";
        let dir = std::env::temp_dir().join(format!(
            "rwa-bridged-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bridged.csv");
        std::fs::write(&path, csv).unwrap();
        let result = sum_bridged_value_from_csv(&path, "2026-06-11").unwrap();
        assert_eq!(result.date, "2026-06-11");
        assert!((result.total_usd - 750_000.0).abs() < 1.0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sum_bridged_value_missing_date_errors() {
        let csv = "Timestamp,Date,Measure,TokenA\n\
                   ts1,2026-06-10,Bridged Token Value (Dollar),100.0\n";
        let dir = std::env::temp_dir().join(format!(
            "rwa-bridged-miss-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bridged.csv");
        std::fs::write(&path, csv).unwrap();
        assert!(sum_bridged_value_from_csv(&path, "2026-06-11").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
