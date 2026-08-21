use serde::Serialize;

use super::value::{Kv3Document, Kv3Value};

const INTERESTING_KEYS: &[&str] = &[
    "m_skeleton",
    "m_modelSkeleton",
    "m_hitboxsets",
    "m_bones",
    "m_boneName",
    "m_nParent",
];
const MAX_FINDINGS: usize = 128;

/// Bounded, read-only observations of specifically named VMDL-shaped keys.
///
/// This is a tree report, not a geometry parser. A finding never establishes
/// model identity, bone transforms, hitbox geometry, or spatial evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VmdlSemanticInspection {
    pub root_class: Option<String>,
    pub findings: Vec<VmdlSemanticFinding>,
    pub exact_geometry_available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VmdlSemanticFinding {
    pub path: String,
    pub key: String,
    pub value_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collection_length: Option<usize>,
}

pub fn inspect_vmdl_semantics(document: &Kv3Document) -> VmdlSemanticInspection {
    let root_class = document
        .root
        .object_field("_class")
        .and_then(Kv3Value::as_str)
        .map(ToOwned::to_owned);
    let mut findings = Vec::new();
    visit(&document.root, "$", &mut findings);
    VmdlSemanticInspection {
        root_class,
        findings,
        exact_geometry_available: false,
    }
}

fn visit(value: &Kv3Value, path: &str, findings: &mut Vec<VmdlSemanticFinding>) {
    match value {
        Kv3Value::Object(fields) => {
            for field in fields {
                let field_path = format!("{path}.{}", field.key);
                if INTERESTING_KEYS.contains(&field.key.as_str()) && findings.len() < MAX_FINDINGS {
                    findings.push(VmdlSemanticFinding {
                        path: field_path.clone(),
                        key: field.key.clone(),
                        value_kind: kind(&field.value).to_owned(),
                        collection_length: collection_length(&field.value),
                    });
                }
                visit(&field.value, &field_path, findings);
            }
        }
        Kv3Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                visit(child, &format!("{path}[{index}]"), findings);
            }
        }
        _ => {}
    }
}

fn kind(value: &Kv3Value) -> &'static str {
    match value {
        Kv3Value::Null => "null",
        Kv3Value::Bool(_) => "bool",
        Kv3Value::Int32(_) => "int32",
        Kv3Value::Int64(_) => "int64",
        Kv3Value::UInt32(_) => "uint32",
        Kv3Value::UInt64(_) => "uint64",
        Kv3Value::Float(_) => "float",
        Kv3Value::Double(_) => "double",
        Kv3Value::String(_) => "string",
        Kv3Value::Array(_) => "array",
        Kv3Value::Object(_) => "object",
        Kv3Value::BinaryBlob(_) => "binary_blob",
    }
}

fn collection_length(value: &Kv3Value) -> Option<usize> {
    match value {
        Kv3Value::Array(values) => Some(values.len()),
        Kv3Value::Object(fields) => Some(fields.len()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::inspect_vmdl_semantics;
    use crate::kv3::{Kv3Document, Kv3Field, Kv3Value};

    #[test]
    fn reports_empty_hitbox_sets_without_promoting_geometry() {
        let document = Kv3Document {
            format_guid: [0; 16],
            root: Kv3Value::Object(vec![
                Kv3Field {
                    key: "_class".to_owned(),
                    value: Kv3Value::String("CRenderMesh".to_owned()),
                },
                Kv3Field {
                    key: "m_hitboxsets".to_owned(),
                    value: Kv3Value::Array(Vec::new()),
                },
            ]),
        };
        let inspection = inspect_vmdl_semantics(&document);
        assert_eq!(inspection.root_class.as_deref(), Some("CRenderMesh"));
        assert_eq!(inspection.findings.len(), 1);
        assert_eq!(inspection.findings[0].collection_length, Some(0));
        assert!(!inspection.exact_geometry_available);
    }
}
