/// Robust z-score using median and MAD (scaled by 1.4826).
pub fn robust_z(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let mut abs_dev: Vec<f64> = values.iter().map(|v| (v - median).abs()).collect();
    abs_dev.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad = abs_dev[abs_dev.len() / 2];
    let denom = 1.4826 * mad;
    if denom < f64::EPSILON {
        return vec![0.0; values.len()];
    }
    values.iter().map(|v| (v - median) / denom).collect()
}

pub fn pearson_r(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() != ys.len() || xs.len() < 2 {
        return None;
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den_x = 0.0;
    let mut den_y = 0.0;
    for i in 0..xs.len() {
        let dx = xs[i] - mean_x;
        let dy = ys[i] - mean_y;
        num += dx * dy;
        den_x += dx * dx;
        den_y += dy * dy;
    }
    let den = (den_x * den_y).sqrt();
    if den < f64::EPSILON {
        None
    } else {
        Some(num / den)
    }
}

pub fn trailing_mean(values: &[f64], window: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(values.len());
    for i in 0..values.len() {
        let start = i.saturating_sub(window - 1);
        let chunk = &values[start..=i];
        out.push(chunk.iter().sum::<f64>() / chunk.len() as f64);
    }
    out
}

pub fn coefficient_of_variation(values: &[f64]) -> Option<f64> {
    let nonzero: Vec<f64> = values.iter().copied().filter(|v| *v > 0.0).collect();
    if nonzero.is_empty() {
        return None;
    }
    let mean = nonzero.iter().sum::<f64>() / nonzero.len() as f64;
    if mean < f64::EPSILON {
        return None;
    }
    let variance = nonzero.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / nonzero.len() as f64;
    Some(variance.sqrt() / mean)
}

pub fn spike_ratio(values: &[f64]) -> Option<f64> {
    let nonzero: Vec<f64> = values.iter().copied().filter(|v| *v > 0.0).collect();
    if nonzero.is_empty() {
        return None;
    }
    let median = {
        let mut s = nonzero.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        s[s.len() / 2]
    };
    let max = nonzero.iter().copied().fold(0.0f64, f64::max);
    if median < f64::EPSILON {
        None
    } else {
        Some(max / median)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robust_z_centers_on_median() {
        let z = robust_z(&[1.0, 2.0, 3.0, 4.0, 100.0]);
        assert_eq!(z.len(), 5);
        let mid = z[2];
        assert!(mid.abs() < 0.5);
    }

    #[test]
    fn pearson_r_perfect_positive() {
        let r = pearson_r(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap();
        assert!((r - 1.0).abs() < 1e-9);
    }

    #[test]
    fn trailing_mean_window_three() {
        let m = trailing_mean(&[1.0, 3.0, 6.0, 10.0], 3);
        assert!((m[2] - (1.0 + 3.0 + 6.0) / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn robust_z_empty_returns_empty() {
        assert!(robust_z(&[]).is_empty());
    }

    #[test]
    fn robust_z_all_same_returns_zeros() {
        let z = robust_z(&[5.0, 5.0, 5.0]);
        assert!(z.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn pearson_r_too_short_returns_none() {
        assert!(pearson_r(&[1.0], &[2.0]).is_none());
        assert!(pearson_r(&[], &[]).is_none());
    }

    #[test]
    fn pearson_r_different_lengths_returns_none() {
        assert!(pearson_r(&[1.0, 2.0], &[1.0]).is_none());
    }

    #[test]
    fn pearson_r_constant_series_returns_none() {
        assert!(pearson_r(&[3.0, 3.0, 3.0], &[1.0, 2.0, 3.0]).is_none());
    }

    #[test]
    fn pearson_r_perfect_negative() {
        let r = pearson_r(&[3.0, 2.0, 1.0], &[1.0, 2.0, 3.0]).unwrap();
        assert!((r + 1.0).abs() < 1e-9);
    }

    #[test]
    fn trailing_mean_window_one_is_identity() {
        let vals = vec![1.0, 5.0, 9.0];
        let m = trailing_mean(&vals, 1);
        assert_eq!(m, vals);
    }

    #[test]
    fn coefficient_of_variation_all_zeros_returns_none() {
        assert!(coefficient_of_variation(&[0.0, 0.0]).is_none());
        assert!(coefficient_of_variation(&[]).is_none());
    }

    #[test]
    fn coefficient_of_variation_constant_nonzero_is_zero() {
        let cv = coefficient_of_variation(&[5.0, 5.0, 5.0]).unwrap();
        assert!(cv.abs() < 1e-10);
    }

    #[test]
    fn coefficient_of_variation_mixed_values() {
        let cv = coefficient_of_variation(&[10.0, 20.0, 30.0]).unwrap();
        assert!(cv > 0.0);
    }

    #[test]
    fn spike_ratio_empty_or_zeros_returns_none() {
        assert!(spike_ratio(&[]).is_none());
        assert!(spike_ratio(&[0.0, 0.0]).is_none());
    }

    #[test]
    fn spike_ratio_single_value() {
        let r = spike_ratio(&[10.0]).unwrap();
        assert!((r - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn spike_ratio_identifies_spike() {
        let r = spike_ratio(&[1.0, 1.0, 1.0, 10.0]).unwrap();
        assert!((r - 10.0).abs() < f64::EPSILON);
    }
}
