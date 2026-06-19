use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use super::types::{Provenance, ProvenanceEnvelope};

pub fn write_json_with_provenance<T: Serialize>(
    path: &Path,
    data: &T,
    provenance: Provenance,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let envelope = ProvenanceEnvelope { provenance, data };
    fs::write(path, serde_json::to_string_pretty(&envelope)? + "\n")?;
    Ok(())
}

pub fn write_json_value_with_provenance(
    path: &Path,
    data: &Value,
    provenance: Provenance,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let envelope = json_envelope(data, provenance);
    fs::write(path, serde_json::to_string_pretty(&envelope)? + "\n")?;
    Ok(())
}

fn json_envelope(data: &Value, provenance: Provenance) -> Value {
    serde_json::json!({
        "provenance": provenance,
        "data": data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::types::SourceId;

    #[test]
    fn provenance_envelope_round_trip_shape() {
        let dir = std::env::temp_dir().join(format!(
            "rwa-prov-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.json");
        let prov = Provenance::new(SourceId::ManualImport, "fixture://test", false);
        write_json_with_provenance(&path, &serde_json::json!({"x": 1}), prov.clone()).unwrap();
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["provenance"]["source"], "manual_import");
        assert_eq!(raw["data"]["x"], 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
