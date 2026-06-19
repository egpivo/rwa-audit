//! Article 3 exchange-surface analysis tools.

mod compression;
mod equivalence;

pub use compression::{surface_compression, PanelMetricRow, RecordSurface};
pub use equivalence::metric_equivalence_check;
