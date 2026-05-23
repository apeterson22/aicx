use std::collections::BTreeMap;

use crate::model::{FileKind, TransformKind};

pub fn apply_transform(
    _kind: FileKind,
    data: &[u8],
) -> (Vec<u8>, TransformKind, BTreeMap<String, String>) {
    (data.to_vec(), TransformKind::Identity, BTreeMap::new())
}

pub fn reverse_transform(
    _transform: TransformKind,
    data: &[u8],
    _metadata: &BTreeMap<String, String>,
) -> Vec<u8> {
    data.to_vec()
}

