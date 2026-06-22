use crate::tools::types::ToolResult;
use crate::tools::TOOL_SENDER_VOLUME_COVERAGE;

#[derive(Debug, Clone, PartialEq)]
pub struct SenderVolume {
    pub symbol: String,
    pub address: String,
    pub volume_usd: f64,
}

/// Article 1 Fig. 3: sender addresses required to explain a volume fraction.
pub fn sender_volume_coverage(senders: &[SenderVolume], fraction: f64) -> ToolResult {
    let result = ToolResult::new(TOOL_SENDER_VOLUME_COVERAGE);
    if senders.is_empty() {
        return result.gap("no sender volume rows");
    }
    if !(0.0..=1.0).contains(&fraction) {
        return result.gap("coverage fraction must be between 0 and 1");
    }

    let total: f64 = senders.iter().map(|s| s.volume_usd).sum();
    if total <= 0.0 {
        return result.gap("total sender volume is zero");
    }

    let mut ranked = senders.to_vec();
    ranked.sort_by(|a, b| {
        b.volume_usd
            .partial_cmp(&a.volume_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let target = total * fraction;
    let mut cumulative = 0.0;
    let mut count = 0usize;
    for sender in &ranked {
        cumulative += sender.volume_usd;
        count += 1;
        if cumulative >= target {
            break;
        }
    }

    let top_share = ranked.first().map(|s| s.volume_usd / total).unwrap_or(0.0);

    result
        .metric("senders_for_coverage", count as f64, "addresses")
        .metric("coverage_fraction", fraction, "ratio")
        .metric("top_sender_share", top_share, "ratio")
        .metric("total_volume_usd", total, "USD")
        .label(format!(
            "{count} senders explain {:.0}% of visible volume",
            fraction * 100.0
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighty_percent_coverage_needs_few_senders_when_concentrated() {
        let senders = vec![
            SenderVolume {
                symbol: "BUIDL".into(),
                address: "0x1".into(),
                volume_usd: 80.0,
            },
            SenderVolume {
                symbol: "BUIDL".into(),
                address: "0x2".into(),
                volume_usd: 15.0,
            },
            SenderVolume {
                symbol: "BUIDL".into(),
                address: "0x3".into(),
                volume_usd: 5.0,
            },
        ];
        let result = sender_volume_coverage(&senders, 0.8);
        let count = result
            .metrics
            .iter()
            .find(|m| m.name == "senders_for_coverage")
            .unwrap()
            .value;
        assert_eq!(count, 1.0);
    }

    #[test]
    fn dispersed_senders_need_more_addresses() {
        let senders: Vec<SenderVolume> = (0..10)
            .map(|i| SenderVolume {
                symbol: "PAXG".into(),
                address: format!("0x{i}"),
                volume_usd: 10.0,
            })
            .collect();
        let result = sender_volume_coverage(&senders, 0.8);
        let count = result
            .metrics
            .iter()
            .find(|m| m.name == "senders_for_coverage")
            .unwrap()
            .value;
        assert_eq!(count, 8.0);
    }
}
