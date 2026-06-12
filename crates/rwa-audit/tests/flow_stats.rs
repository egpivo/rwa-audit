use rwa_audit::flow::stats::{coefficient_of_variation, pearson_r, robust_z, spike_ratio};

#[test]
fn fragility_metrics_on_skewed_volume() {
    let vols = vec![1000.0, 1200.0, 800.0, 1500.0, 50_000.0];
    let cv = coefficient_of_variation(&vols).unwrap();
    let spike = spike_ratio(&vols).unwrap();
    assert!(cv > 1.0);
    assert!(spike > 10.0);
}

#[test]
fn robust_z_and_correlation_pipeline() {
    let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let ys = vec![5.0, 4.0, 3.0, 2.0, 1.0];
    let zx = robust_z(&xs);
    let zy = robust_z(&ys);
    assert_eq!(zx.len(), 5);
    let r = pearson_r(&zx, &zy).unwrap();
    assert!(r < -0.9);
    let _ = zy;
}
