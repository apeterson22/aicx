use anyhow::Result;
use pyo3::prelude::*;
use serde::Serialize;
use std::path::PathBuf;

use aicx_core::{
    archive_digests as archive_digests_core, compare_profiles as compare_profiles_core,
    extract_archive, inspect_archive, list_archive_paths, manifest_digest as manifest_digest_core,
    pack_archive, render_toon as render_toon_core, report_archive,
    sidecar_digest as sidecar_digest_core, verify_archive, ArchiveProfile, HashAlgorithm,
    PackOptions, Selection,
};

fn json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

fn toon<T: Serialize>(value: &T) -> Result<String> {
    Ok(render_toon_core(value)?)
}

fn parse_profile(value: Option<String>) -> Result<ArchiveProfile> {
    value
        .as_deref()
        .unwrap_or("balanced")
        .parse()
        .map_err(anyhow::Error::msg)
}

fn parse_hash(value: Option<String>) -> Result<HashAlgorithm> {
    value
        .as_deref()
        .unwrap_or("blake3")
        .parse()
        .map_err(anyhow::Error::msg)
}

#[pyfunction]
fn pack(
    inputs: Vec<String>,
    out: String,
    profile: Option<String>,
    chunk_size: Option<usize>,
    hash: Option<String>,
) -> PyResult<String> {
    let options = PackOptions {
        chunk_size: chunk_size.unwrap_or(4 * 1024 * 1024),
        profile: parse_profile(profile)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?,
        hash_algorithm: parse_hash(hash)
            .map_err(|err| pyo3::exceptions::PyValueError::new_err(err.to_string()))?,
    };
    let inputs = inputs.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    let manifest = pack_archive(&inputs, &PathBuf::from(out), options)
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&manifest).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn unpack(
    archive: String,
    out: String,
    paths: Option<Vec<String>>,
    exact: Option<bool>,
    overwrite: Option<bool>,
) -> PyResult<String> {
    let selection = Selection {
        paths: paths.unwrap_or_default(),
        exact: exact.unwrap_or(false),
    };
    let manifest = extract_archive(
        &PathBuf::from(archive),
        &PathBuf::from(out),
        selection,
        overwrite.unwrap_or(false),
    )
    .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&manifest).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn extract(
    archive: String,
    path: String,
    out: String,
    exact: Option<bool>,
    overwrite: Option<bool>,
) -> PyResult<String> {
    unpack(archive, out, Some(vec![path]), exact, overwrite)
}

#[pyfunction]
fn inspect(archive: String) -> PyResult<String> {
    let inspection = inspect_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&inspection).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn inspect_toon(archive: String) -> PyResult<String> {
    let inspection = inspect_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    toon(&inspection).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn list_paths(archive: String) -> PyResult<Vec<String>> {
    list_archive_paths(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn verify(archive: String) -> PyResult<String> {
    let verification = verify_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&verification).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn report(archive: String) -> PyResult<String> {
    let report = report_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&report).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn report_toon(archive: String) -> PyResult<String> {
    let report = report_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    toon(&report).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn sidecar(archive: String) -> PyResult<String> {
    let inspection = inspect_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&inspection.sidecar)
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn sidecar_toon(archive: String) -> PyResult<String> {
    let inspection = inspect_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    toon(&inspection.sidecar)
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn compare_profiles(inputs: Vec<String>, chunk_size: Option<usize>) -> PyResult<String> {
    let inputs = inputs.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    let rows = compare_profiles_core(&inputs, chunk_size.unwrap_or(4 * 1024 * 1024))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&rows).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn digest(archive: String) -> PyResult<String> {
    let output = archive_digests_core(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    json(&output).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn digest_toon(archive: String) -> PyResult<String> {
    let output = archive_digests_core(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    toon(&output).map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn manifest_digest(archive: String) -> PyResult<String> {
    let inspection = inspect_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    manifest_digest_core(&inspection.manifest)
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pyfunction]
fn sidecar_digest(archive: String) -> PyResult<String> {
    let inspection = inspect_archive(&PathBuf::from(archive))
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))?;
    sidecar_digest_core(&inspection.sidecar)
        .map_err(|err| pyo3::exceptions::PyRuntimeError::new_err(err.to_string()))
}

#[pymodule]
fn aicx_native(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(pack, module)?)?;
    module.add_function(wrap_pyfunction!(unpack, module)?)?;
    module.add_function(wrap_pyfunction!(extract, module)?)?;
    module.add_function(wrap_pyfunction!(inspect, module)?)?;
    module.add_function(wrap_pyfunction!(inspect_toon, module)?)?;
    module.add_function(wrap_pyfunction!(list_paths, module)?)?;
    module.add_function(wrap_pyfunction!(verify, module)?)?;
    module.add_function(wrap_pyfunction!(report, module)?)?;
    module.add_function(wrap_pyfunction!(report_toon, module)?)?;
    module.add_function(wrap_pyfunction!(sidecar, module)?)?;
    module.add_function(wrap_pyfunction!(sidecar_toon, module)?)?;
    module.add_function(wrap_pyfunction!(compare_profiles, module)?)?;
    module.add_function(wrap_pyfunction!(digest, module)?)?;
    module.add_function(wrap_pyfunction!(digest_toon, module)?)?;
    module.add_function(wrap_pyfunction!(manifest_digest, module)?)?;
    module.add_function(wrap_pyfunction!(sidecar_digest, module)?)?;
    Ok(())
}
