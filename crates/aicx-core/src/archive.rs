use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use blake3::Hasher;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

use crate::chunking::fixed_size_chunks;
use crate::classify::{classify, detect_language};
use crate::compression::{decompress, select_codec};
use crate::error::{AicxError, Result};
use crate::model::*;
use crate::pathing::{
    archive_path_to_string, ensure_directory_chain_safe, ensure_parent_chain_safe,
    is_filesystem_root, normalize_directory_path, safe_output_path, validate_archive_path,
};
use crate::sidecar::build_sidecar;
use crate::transform::{apply_transform, reverse_transform};

const MAGIC: &[u8; 5] = b"AICX2";
const VERSION: u8 = 2;
const CREATED_BY: &str = "aicx-rust";
static TEMPFILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
struct ArchiveIdentitySeed<'a> {
    magic: &'a str,
    version: u32,
    created_by: &'a str,
    deterministic_build: bool,
    profile: ArchiveProfile,
    chunk_size: u64,
    original_total_size: u64,
    stored_total_size: u64,
    compression_ratio: f64,
    file_count: u64,
    chunk_count: u64,
    hash_algorithm: HashAlgorithm,
    data_hash: &'a str,
    files: &'a [FileRecord],
    chunks: &'a [ChunkRecord],
}

#[derive(Debug, Clone)]
struct SourceFile {
    archive_path: String,
    original_path: Option<String>,
    bytes: Vec<u8>,
    executable_hint: bool,
}

fn hash_bytes(algorithm: HashAlgorithm, data: &[u8]) -> String {
    match algorithm {
        HashAlgorithm::Blake3 => blake3::hash(data).to_hex().to_string(),
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            format!("{:x}", hasher.finalize())
        }
    }
}

fn archive_identity_seed<'a>(manifest: &'a ArchiveManifest) -> ArchiveIdentitySeed<'a> {
    ArchiveIdentitySeed {
        magic: &manifest.magic,
        version: manifest.version,
        created_by: &manifest.created_by,
        deterministic_build: manifest.deterministic_build,
        profile: manifest.profile,
        chunk_size: manifest.chunk_size,
        original_total_size: manifest.original_total_size,
        stored_total_size: manifest.stored_total_size,
        compression_ratio: manifest.compression_ratio,
        file_count: manifest.file_count,
        chunk_count: manifest.chunk_count,
        hash_algorithm: manifest.hash_algorithm,
        data_hash: &manifest.data_hash,
        files: &manifest.files,
        chunks: &manifest.chunks,
    }
}

fn compute_archive_id(manifest: &ArchiveManifest) -> Result<String> {
    manifest_digest(manifest)
}

fn validate_archive_id(manifest: &ArchiveManifest) -> Result<()> {
    let expected_archive_id = compute_archive_id(manifest)?;
    if manifest.archive_id != expected_archive_id {
        return Err(AicxError::Validation(
            "archive id does not match manifest contents".to_string(),
        ));
    }
    Ok(())
}

fn resolve_runtime_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

pub fn manifest_digest(manifest: &ArchiveManifest) -> Result<String> {
    let seed = archive_identity_seed(manifest);
    let serialized = serde_json::to_string(&seed)?;
    Ok(blake3::hash(serialized.as_bytes()).to_hex().to_string())
}

pub fn sidecar_digest(sidecar: &ArchiveSidecar) -> Result<String> {
    let serialized = serde_json::to_string(sidecar)?;
    Ok(blake3::hash(serialized.as_bytes()).to_hex().to_string())
}

fn archive_path_from_root(root_name: &str, relative_path: &Path) -> Result<String> {
    if relative_path.to_str().is_none() {
        return Err(AicxError::UnsafePath(
            "non-UTF-8 archive path rejected".to_string(),
        ));
    }
    let mut combined = PathBuf::from(root_name);
    combined.push(relative_path);
    let archive_path = archive_path_to_string(&combined);
    validate_archive_path(&archive_path)?;
    Ok(archive_path)
}

fn validate_original_path_metadata(original_path: &str) -> Result<()> {
    let normalized = validate_archive_path(original_path)?;
    let canonical = archive_path_to_string(&normalized);
    if canonical != original_path {
        return Err(AicxError::Validation(format!(
            "non-canonical original path metadata rejected: {original_path}"
        )));
    }
    Ok(())
}

fn root_name_for_input(input: &Path) -> Result<String> {
    if let Some(name) = input
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
    {
        return Ok(name.to_string());
    }

    let canonical = fs::canonicalize(input)?;
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != "." && *name != "..")
        .ok_or_else(|| {
            AicxError::UnsafePath(format!(
                "input path has no usable archive root name: {}",
                input.display()
            ))
        })?;
    Ok(name.to_string())
}

fn is_executable_hint(path: &Path, bytes: &[u8]) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            if meta.permissions().mode() & 0o111 != 0 {
                return true;
            }
        }
    }
    bytes.starts_with(b"#!")
}

fn gather_source_files(inputs: &[PathBuf]) -> Result<Vec<SourceFile>> {
    let base_dir = fs::canonicalize(".")?;
    let mut files = Vec::new();

    for input in inputs {
        let fs_input = resolve_runtime_path(&base_dir, input);
        let metadata = fs::symlink_metadata(&fs_input)?;
        if metadata.file_type().is_symlink() {
            return Err(AicxError::UnsafePath(format!(
                "refusing to pack symlink source: {}",
                input.display()
            )));
        }
        if metadata.is_dir() {
            let root_name = root_name_for_input(&fs_input)?;
            for entry in WalkDir::new(&fs_input).follow_links(false) {
                let entry = entry.map_err(|err| AicxError::Validation(err.to_string()))?;
                if entry.path() == fs_input {
                    continue;
                }
                let entry_type = entry.file_type();
                if entry_type.is_symlink() {
                    return Err(AicxError::UnsafePath(format!(
                        "refusing to pack symlink source: {}",
                        entry.path().display()
                    )));
                }
                if entry_type.is_file() {
                    let relative = entry.path().strip_prefix(&fs_input).map_err(|_| {
                        AicxError::Validation("failed to compute relative path".to_string())
                    })?;
                    let archive_path = archive_path_from_root(&root_name, relative)?;
                    let bytes = fs::read(entry.path())?;
                    files.push(SourceFile {
                        original_path: Some(archive_path.clone()),
                        archive_path,
                        executable_hint: is_executable_hint(entry.path(), &bytes),
                        bytes,
                    });
                }
            }
        } else if metadata.is_file() {
            let root_name = root_name_for_input(&fs_input)?;
            let archive_path = archive_path_from_root(&root_name, Path::new(""))?;
            let bytes = fs::read(&fs_input)?;
            let original_path = if input.is_absolute() {
                input
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
            } else {
                normalize_directory_path(input)
                    .ok()
                    .map(|normalized| archive_path_to_string(&normalized))
            };
            files.push(SourceFile {
                archive_path,
                original_path,
                executable_hint: is_executable_hint(&fs_input, &bytes),
                bytes,
            });
        } else {
            return Err(AicxError::Validation(format!(
                "unsupported input path type: {}",
                input.display()
            )));
        }
    }

    files.sort_by(|left, right| left.archive_path.cmp(&right.archive_path));
    Ok(files)
}

fn build_from_sources(
    inputs: &[PathBuf],
    options: &PackOptions,
) -> Result<(ArchiveEnvelope, Vec<u8>)> {
    if options.chunk_size == 0 {
        return Err(AicxError::Validation(
            "chunk size must be positive".to_string(),
        ));
    }
    let source_files = gather_source_files(inputs)?;
    if source_files.is_empty() {
        return Err(AicxError::Validation("no input files found".to_string()));
    }
    let mut seen_archive_paths = std::collections::BTreeSet::new();
    for source in &source_files {
        if !seen_archive_paths.insert(source.archive_path.clone()) {
            return Err(AicxError::Validation(format!(
                "duplicate archive path produced during pack: {}",
                source.archive_path
            )));
        }
    }
    let mut chunk_records = Vec::new();
    let mut file_records = Vec::new();
    let mut chunk_data = Vec::new();
    let mut data_hasher = Hasher::new();
    let mut original_total_size = 0u64;

    for (file_id, source) in source_files.iter().enumerate() {
        let file_kind = classify(&source.archive_path, &source.bytes);
        let file_hash = hash_bytes(options.hash_algorithm, &source.bytes);
        let detected_language = detect_language(&source.archive_path);
        let file_chunks = fixed_size_chunks(&source.bytes, options.chunk_size);
        let mut chunk_refs = Vec::new();
        let mut restored_size = 0u64;
        let mut file_risks = Vec::new();
        if source.archive_path.to_ascii_lowercase().contains(".env") {
            file_risks.push("contains .env file".to_string());
        }
        if source.archive_path.to_ascii_lowercase().contains("key") {
            file_risks.push("contains private key pattern".to_string());
        }
        if source.bytes.len() > 10 * 1024 * 1024 {
            file_risks.push("contains large file".to_string());
        }

        for (chunk_index, chunk) in file_chunks.iter().enumerate() {
            let classification = file_kind;
            let (transformed, transform, transform_metadata) =
                apply_transform(classification, chunk);
            let (codec, codec_level, compressed) =
                select_codec(options.profile, classification, &transformed)?;
            let chunk_hash = hash_bytes(options.hash_algorithm, chunk);
            let stored_size = compressed.len() as u64;
            let offset = chunk_data.len() as u64;
            chunk_data.extend_from_slice(&compressed);
            data_hasher.update(&compressed);
            let original_size = chunk.len() as u64;
            restored_size += original_size;
            chunk_records.push(ChunkRecord {
                chunk_id: chunk_records.len() as u64,
                file_id: file_id as u64,
                chunk_index: chunk_index as u64,
                original_size,
                stored_size,
                codec,
                codec_level,
                transform,
                hash: chunk_hash,
                offset,
                length: stored_size,
                classification,
                compression_ratio: if stored_size == 0 {
                    1.0
                } else {
                    original_size as f64 / stored_size as f64
                },
                transform_metadata,
            });
            chunk_refs.push((chunk_records.len() - 1) as u64);
        }

        original_total_size += source.bytes.len() as u64;
        file_records.push(FileRecord {
            archive_path: source.archive_path.clone(),
            original_path: source.original_path.clone(),
            original_size: source.bytes.len() as u64,
            restored_size,
            file_hash,
            file_type: file_kind,
            detected_language,
            transform_applied: TransformKind::Identity,
            chunk_refs,
            executable_hint: source.executable_hint,
            risk_hints: file_risks,
            metadata_flags: vec!["deterministic_build".to_string()],
        });
    }

    let stored_total_size = chunk_data.len() as u64;
    let compression_ratio = if stored_total_size == 0 {
        1.0
    } else {
        original_total_size as f64 / stored_total_size as f64
    };
    let data_hash = data_hasher.finalize().to_hex().to_string();
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: VERSION as u32,
        archive_id: String::new(),
        created_by: CREATED_BY.to_string(),
        deterministic_build: true,
        profile: options.profile,
        chunk_size: options.chunk_size as u64,
        original_total_size,
        stored_total_size,
        compression_ratio,
        file_count: file_records.len() as u64,
        chunk_count: chunk_records.len() as u64,
        hash_algorithm: options.hash_algorithm,
        data_hash,
        files: file_records,
        chunks: chunk_records,
    };
    manifest.archive_id = compute_archive_id(&manifest)?;
    let sidecar = build_sidecar(&manifest);
    Ok((ArchiveEnvelope { manifest, sidecar }, chunk_data))
}

fn write_archive(output: &Path, envelope: &ArchiveEnvelope, data: &[u8]) -> Result<()> {
    let base_dir = fs::canonicalize(".")?;
    let output = resolve_runtime_path(&base_dir, output);
    let file_name = match output.components().next_back() {
        Some(Component::Normal(file_name)) => file_name,
        Some(Component::CurDir | Component::ParentDir) => {
            return Err(AicxError::UnsafePath(format!(
                "unsafe archive path rejected: {}",
                output.display()
            )));
        }
        Some(Component::RootDir | Component::Prefix(_)) | None => {
            return Err(AicxError::UnsafePath(
                "archive output has no file name".to_string(),
            ));
        }
    };
    let file_name = file_name.to_str().ok_or_else(|| {
        AicxError::UnsafePath("archive output filename must be UTF-8".to_string())
    })?;
    validate_archive_path(file_name)?;
    let output_path = if let Some(parent) = output.parent() {
        ensure_directory_chain_safe(parent)?;
        let normalized_parent = normalize_directory_path(parent)?;
        let canonical_parent = if normalized_parent.as_os_str().is_empty() {
            fs::canonicalize(".")?
        } else {
            fs::canonicalize(&normalized_parent)?
        };
        if is_filesystem_root(&canonical_parent) {
            return Err(AicxError::UnsafePath(
                "archive output may not target filesystem root".to_string(),
            ));
        }
        ensure_existing_target_safe(&canonical_parent.join(file_name))?;
        canonical_parent.join(file_name)
    } else {
        return Err(AicxError::UnsafePath(
            "archive output has no parent".to_string(),
        ));
    };
    let manifest_bytes = serde_cbor::to_vec(envelope)?;
    let mut temp_file = new_temp_file(
        output_path
            .parent()
            .ok_or_else(|| AicxError::UnsafePath("archive output has no parent".to_string()))?,
        &output_path,
    )?;
    {
        let mut writer = BufWriter::new(temp_file.as_file_mut());
        writer.write_all(MAGIC)?;
        writer.write_all(&[VERSION])?;
        writer.write_all(&(manifest_bytes.len() as u64).to_be_bytes())?;
        writer.write_all(&manifest_bytes)?;
        writer.write_all(data)?;
        writer.flush()?;
    }
    temp_file.as_file_mut().sync_all()?;
    persist_temp_file(temp_file, &output_path, false)?;
    Ok(())
}

fn read_archive(archive: &Path) -> Result<(ArchiveEnvelope, Vec<u8>)> {
    let file = File::open(archive)?;
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; 5];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(AicxError::Validation(
            "invalid AICX magic bytes".to_string(),
        ));
    }
    let mut version = [0u8; 1];
    reader.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(AicxError::Validation(format!(
            "unsupported archive version: {}",
            version[0]
        )));
    }
    let mut header_len = [0u8; 8];
    reader.read_exact(&mut header_len)?;
    let header_len = u64::from_be_bytes(header_len);
    let metadata = fs::metadata(archive)?;
    let total_size = metadata.len();
    let prefix_size = 5 + 1 + 8;
    if header_len > total_size.saturating_sub(prefix_size as u64) {
        return Err(AicxError::Validation(
            "archive header length exceeds archive size".to_string(),
        ));
    }
    let header_len = usize::try_from(header_len).map_err(|_| {
        AicxError::Validation("archive header length exceeds host limits".to_string())
    })?;
    let mut header_bytes = vec![0u8; header_len];
    reader.read_exact(&mut header_bytes)?;
    let envelope: ArchiveEnvelope = serde_cbor::from_slice(&header_bytes)?;
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    Ok((envelope, data))
}

fn ensure_existing_target_safe(path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(AicxError::UnsafePath(format!(
                "refusing to follow symlink: {}",
                path.display()
            )));
        }
        if !metadata.file_type().is_file() {
            return Err(AicxError::UnsafePath(format!(
                "refusing to write to non-regular file: {}",
                path.display()
            )));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            if metadata.nlink() > 1 {
                return Err(AicxError::UnsafePath(format!(
                    "refusing to overwrite hard-linked file: {}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn new_temp_file(parent: &Path, final_path: &Path) -> Result<NamedTempFile> {
    let temp_index = TEMPFILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let final_name = final_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("aicx");
    let prefix = format!(".{final_name}.tmp.{temp_index}.");
    Ok(tempfile::Builder::new()
        .prefix(&prefix)
        .tempfile_in(parent)?)
}

fn write_temp_file(temp_file: &mut NamedTempFile, data: &[u8]) -> Result<()> {
    {
        let mut writer = BufWriter::new(temp_file.as_file_mut());
        writer.write_all(data)?;
        writer.flush()?;
    }
    temp_file.as_file_mut().sync_all()?;
    Ok(())
}

fn apply_restore_permissions(temp_file: &NamedTempFile, executable_hint: bool) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = if executable_hint { 0o755 } else { 0o644 };
        let mut permissions = temp_file.as_file().metadata()?.permissions();
        permissions.set_mode(mode);
        temp_file.as_file().set_permissions(permissions)?;
    }
    #[cfg(not(unix))]
    let _ = executable_hint;
    Ok(())
}

fn persist_temp_file(temp_file: NamedTempFile, path: &Path, overwrite: bool) -> Result<()> {
    let result = if overwrite {
        temp_file.persist(path)
    } else {
        temp_file.persist_noclobber(path)
    };

    match result {
        Ok(_) => Ok(()),
        Err(err) => {
            if !overwrite && err.error.kind() == ErrorKind::AlreadyExists {
                Err(AicxError::OverwriteDenied(path.display().to_string()))
            } else {
                Err(err.error.into())
            }
        }
    }
}

fn validate_sidecar(manifest: &ArchiveManifest, sidecar: &ArchiveSidecar) -> Result<()> {
    if sidecar.summary.total_files != manifest.file_count {
        return Err(AicxError::Validation(
            "sidecar total file count does not match manifest".to_string(),
        ));
    }
    if sidecar.summary.total_size != manifest.original_total_size {
        return Err(AicxError::Validation(
            "sidecar total size does not match manifest".to_string(),
        ));
    }
    if sidecar.summary.compressed_size != manifest.stored_total_size {
        return Err(AicxError::Validation(
            "sidecar compressed size does not match manifest".to_string(),
        ));
    }
    if sidecar.summary.compression_ratio != manifest.compression_ratio {
        return Err(AicxError::Validation(
            "sidecar compression ratio does not match manifest".to_string(),
        ));
    }

    let mut expected_file_types = BTreeMap::new();
    for file in &manifest.files {
        *expected_file_types
            .entry(file.file_type.to_string())
            .or_insert(0) += 1;
    }
    if sidecar.summary.file_type_counts != expected_file_types {
        return Err(AicxError::Validation(
            "sidecar file type counts do not match manifest".to_string(),
        ));
    }

    let mut expected_codec_usage = BTreeMap::new();
    let mut expected_transform_usage = BTreeMap::new();
    for chunk in &manifest.chunks {
        *expected_codec_usage
            .entry(chunk.codec.to_string())
            .or_insert(0) += 1;
        *expected_transform_usage
            .entry(chunk.transform.to_string())
            .or_insert(0) += 1;
    }
    if sidecar.summary.codec_usage != expected_codec_usage {
        return Err(AicxError::Validation(
            "sidecar codec usage does not match manifest".to_string(),
        ));
    }
    if sidecar.summary.transform_usage != expected_transform_usage {
        return Err(AicxError::Validation(
            "sidecar transform usage does not match manifest".to_string(),
        ));
    }

    if sidecar.file_summaries.len() != manifest.files.len() {
        return Err(AicxError::Validation(
            "sidecar file summaries do not match manifest".to_string(),
        ));
    }
    if sidecar.extraction_map.len() != manifest.files.len() {
        return Err(AicxError::Validation(
            "sidecar extraction map does not match manifest".to_string(),
        ));
    }
    for (idx, summary) in sidecar.file_summaries.iter().enumerate() {
        let file = &manifest.files[idx];
        if summary.archive_path != file.archive_path {
            return Err(AicxError::Validation(format!(
                "sidecar file summary {idx} archive path mismatch"
            )));
        }
        if summary.file_type != file.file_type {
            return Err(AicxError::Validation(format!(
                "sidecar file summary {idx} file type mismatch"
            )));
        }
        if summary.size != file.original_size {
            return Err(AicxError::Validation(format!(
                "sidecar file summary {idx} size mismatch"
            )));
        }
        if summary.hash != file.file_hash {
            return Err(AicxError::Validation(format!(
                "sidecar file summary {idx} hash mismatch"
            )));
        }
        if summary.chunk_count != file.chunk_refs.len() as u64 {
            return Err(AicxError::Validation(format!(
                "sidecar file summary {idx} chunk count mismatch"
            )));
        }
        if let Some(extraction) = sidecar.extraction_map.get(idx) {
            if extraction.archive_path != file.archive_path {
                return Err(AicxError::Validation(format!(
                    "sidecar extraction hint {idx} archive path mismatch"
                )));
            }
            if extraction.chunk_refs != file.chunk_refs {
                return Err(AicxError::Validation(format!(
                    "sidecar extraction hint {idx} chunk refs mismatch"
                )));
            }
        }
    }

    let expected_sidecar = build_sidecar(manifest);
    if sidecar.entrypoint_candidates != expected_sidecar.entrypoint_candidates
        || sidecar.dependency_hints != expected_sidecar.dependency_hints
        || sidecar.risk_hints != expected_sidecar.risk_hints
        || sidecar.query_hints != expected_sidecar.query_hints
        || sidecar.aegisqr_integration_hints != expected_sidecar.aegisqr_integration_hints
    {
        return Err(AicxError::Validation(
            "sidecar contents do not match deterministic rebuild".to_string(),
        ));
    }

    Ok(())
}

fn validate_archive(envelope: &ArchiveEnvelope, data: &[u8]) -> Result<()> {
    let manifest = &envelope.manifest;
    if manifest.magic != "AICX2" {
        return Err(AicxError::Validation("manifest magic mismatch".to_string()));
    }
    if manifest.version != VERSION as u32 {
        return Err(AicxError::Validation(format!(
            "unsupported manifest version: {}",
            manifest.version
        )));
    }
    if manifest.chunk_size == 0 {
        return Err(AicxError::Validation(
            "chunk size must be positive".to_string(),
        ));
    }
    let file_count = usize::try_from(manifest.file_count)
        .map_err(|_| AicxError::Validation("file count exceeds host limits".to_string()))?;
    if file_count != manifest.files.len() {
        return Err(AicxError::Validation(
            "file count does not match manifest".to_string(),
        ));
    }
    let chunk_count = usize::try_from(manifest.chunk_count)
        .map_err(|_| AicxError::Validation("chunk count exceeds host limits".to_string()))?;
    if chunk_count != manifest.chunks.len() {
        return Err(AicxError::Validation(
            "chunk count does not match manifest".to_string(),
        ));
    }
    let stored_total_size = usize::try_from(manifest.stored_total_size)
        .map_err(|_| AicxError::Validation("stored total size exceeds host limits".to_string()))?;
    if stored_total_size != data.len() {
        return Err(AicxError::Validation(
            "stored size does not match chunk data length".to_string(),
        ));
    }
    let actual_original_total_size = manifest.files.iter().try_fold(0u64, |acc, file| {
        acc.checked_add(file.original_size).ok_or_else(|| {
            AicxError::Validation("original total size overflows validation".to_string())
        })
    })?;
    if actual_original_total_size != manifest.original_total_size {
        return Err(AicxError::Validation(
            "original size does not match file table".to_string(),
        ));
    }
    let actual_data_hash = hash_bytes(manifest.hash_algorithm, data);
    if actual_data_hash != manifest.data_hash {
        return Err(AicxError::Validation(
            "chunk data hash does not match manifest".to_string(),
        ));
    }
    validate_sidecar(manifest, &envelope.sidecar)?;

    let mut expected_offset = 0u64;
    let mut previous_chunk_file_id: Option<u64> = None;
    let mut previous_chunk_index: Option<u64> = None;
    for (idx, chunk) in manifest.chunks.iter().enumerate() {
        if chunk.chunk_id != idx as u64 {
            return Err(AicxError::Validation(format!(
                "chunk {idx} has unexpected chunk_id {}",
                chunk.chunk_id
            )));
        }
        if let Some(previous_file_id) = previous_chunk_file_id {
            if chunk.file_id < previous_file_id {
                return Err(AicxError::Validation(format!(
                    "chunks are not sorted by file id: {previous_file_id} before {}",
                    chunk.file_id
                )));
            }
            if chunk.file_id == previous_file_id {
                if let Some(previous_index) = previous_chunk_index {
                    if chunk.chunk_index <= previous_index {
                        return Err(AicxError::Validation(format!(
                            "chunk indices are not strictly increasing within file {}",
                            chunk.file_id
                        )));
                    }
                }
            } else if chunk.chunk_index != 0 {
                return Err(AicxError::Validation(format!(
                    "chunk indices must restart at 0 when file changes: file {} has index {}",
                    chunk.file_id, chunk.chunk_index
                )));
            }
        } else if chunk.chunk_index != 0 {
            return Err(AicxError::Validation(format!(
                "first chunk must start at index 0, got {}",
                chunk.chunk_index
            )));
        }
        if !matches!(
            chunk.codec,
            CodecKind::None | CodecKind::Zstd | CodecKind::Lz4 | CodecKind::Gzip | CodecKind::Xz
        ) {
            return Err(AicxError::Validation(format!(
                "chunk {idx} uses unsupported codec {:?}",
                chunk.codec
            )));
        }
        if !matches!(chunk.transform, TransformKind::Identity) {
            return Err(AicxError::Validation(format!(
                "chunk {idx} uses unsupported transform {:?}",
                chunk.transform
            )));
        }
        let file_index = usize::try_from(chunk.file_id).map_err(|_| {
            AicxError::Validation(format!("chunk {idx} file_id overflows host usize"))
        })?;
        if file_index >= manifest.files.len() {
            return Err(AicxError::Validation(format!(
                "chunk {idx} references missing file {}",
                chunk.file_id
            )));
        }
        if chunk.stored_size != chunk.length {
            return Err(AicxError::Validation(format!(
                "chunk {idx} stored size does not match chunk length"
            )));
        }
        let offset = usize::try_from(chunk.offset).map_err(|_| {
            AicxError::Validation(format!("chunk {idx} offset overflows host usize"))
        })?;
        let length = usize::try_from(chunk.length).map_err(|_| {
            AicxError::Validation(format!("chunk {idx} length overflows host usize"))
        })?;
        if chunk.offset != expected_offset {
            return Err(AicxError::Validation(format!(
                "chunk {idx} offset is not contiguous with prior chunk data"
            )));
        }
        let end = offset
            .checked_add(length)
            .ok_or_else(|| AicxError::Validation(format!("chunk {idx} range overflows")))?;
        if end > data.len() {
            return Err(AicxError::Validation(format!(
                "chunk {idx} exceeds available chunk data"
            )));
        }
        expected_offset = chunk.offset.checked_add(chunk.length).ok_or_else(|| {
            AicxError::Validation(format!("chunk {idx} offset plus length overflows"))
        })?;
        previous_chunk_file_id = Some(chunk.file_id);
        previous_chunk_index = Some(chunk.chunk_index);
    }
    if expected_offset != manifest.stored_total_size {
        return Err(AicxError::Validation(
            "chunk offsets do not cover the stored payload exactly".to_string(),
        ));
    }

    let mut seen_archive_paths = std::collections::BTreeSet::new();
    let mut previous_archive_path: Option<&str> = None;
    let mut referenced_chunks = vec![false; manifest.chunks.len()];
    for (file_idx, file) in manifest.files.iter().enumerate() {
        if let Some(previous) = previous_archive_path {
            if previous > file.archive_path.as_str() {
                return Err(AicxError::Validation(format!(
                    "files are not sorted by archive path: {previous} before {}",
                    file.archive_path
                )));
            }
        }
        validate_archive_path(&file.archive_path)?;
        if !seen_archive_paths.insert(file.archive_path.clone()) {
            return Err(AicxError::Validation(format!(
                "duplicate archive path in manifest: {}",
                file.archive_path
            )));
        }
        if let Some(original_path) = &file.original_path {
            validate_original_path_metadata(original_path)?;
        }
        if file.original_size != file.restored_size {
            return Err(AicxError::Validation(format!(
                "file {file_idx} has mismatched original and restored sizes"
            )));
        }
        if file.original_size > 0 && file.chunk_refs.is_empty() {
            return Err(AicxError::Validation(format!(
                "file {file_idx} has content but no chunk references"
            )));
        }
        let mut reconstructed_size = 0u64;
        let mut previous_chunk_ref: Option<u64> = None;
        for (position, chunk_ref) in file.chunk_refs.iter().enumerate() {
            let chunk_index = usize::try_from(*chunk_ref).map_err(|_| {
                AicxError::Validation(format!(
                    "file {file_idx} chunk reference {position} overflows host usize"
                ))
            })?;
            if chunk_index >= manifest.chunks.len() {
                return Err(AicxError::Validation(format!(
                    "file {file_idx} references missing chunk {chunk_index}"
                )));
            }
            if referenced_chunks[chunk_index] {
                return Err(AicxError::Validation(format!(
                    "chunk {chunk_index} is referenced more than once"
                )));
            }
            if let Some(previous) = previous_chunk_ref {
                if previous >= *chunk_ref {
                    return Err(AicxError::Validation(format!(
                        "file {file_idx} chunk references are not strictly increasing"
                    )));
                }
            }
            let chunk = manifest.chunks.get(chunk_index).ok_or_else(|| {
                AicxError::Validation(format!(
                    "file {file_idx} references missing chunk {chunk_index}"
                ))
            })?;
            if chunk.file_id != file_idx as u64 {
                return Err(AicxError::Validation(format!(
                    "chunk ownership does not match file {file_idx}: chunk {chunk_index} belongs to file {}",
                    chunk.file_id
                )));
            }
            referenced_chunks[chunk_index] = true;
            if chunk.chunk_index != position as u64 {
                return Err(AicxError::Validation(format!(
                    "chunk index does not match file {file_idx} position {position}"
                )));
            }
            reconstructed_size = reconstructed_size
                .checked_add(chunk.original_size)
                .ok_or_else(|| {
                    AicxError::Validation(format!("file {file_idx} size overflows validation"))
                })?;
            previous_chunk_ref = Some(*chunk_ref);
        }
        if reconstructed_size != file.original_size {
            return Err(AicxError::Validation(format!(
                "file {file_idx} size mismatch: manifest={} reconstructed={}",
                file.original_size, reconstructed_size
            )));
        }
        previous_archive_path = Some(file.archive_path.as_str());
    }

    if let Some((chunk_index, _)) = referenced_chunks
        .iter()
        .enumerate()
        .find(|(_, referenced)| !**referenced)
    {
        return Err(AicxError::Validation(format!(
            "unreferenced chunk {chunk_index} remains in manifest"
        )));
    }

    Ok(())
}

pub fn pack_archive(
    inputs: &[PathBuf],
    output: &Path,
    options: PackOptions,
) -> Result<ArchiveManifest> {
    let (envelope, data) = build_from_sources(inputs, &options)?;
    write_archive(output, &envelope, &data)?;
    Ok(envelope.manifest)
}

pub fn inspect_archive(archive: &Path) -> Result<ArchiveInspection> {
    let (envelope, data) = read_archive(archive)?;
    validate_archive(&envelope, &data)?;
    validate_archive_id(&envelope.manifest)?;
    let manifest_digest = manifest_digest(&envelope.manifest)?;
    let sidecar_digest = sidecar_digest(&envelope.sidecar)?;
    Ok(ArchiveInspection {
        manifest: envelope.manifest,
        sidecar: envelope.sidecar,
        manifest_digest,
        sidecar_digest,
    })
}

pub fn list_archive_paths(archive: &Path) -> Result<Vec<String>> {
    let inspection = inspect_archive(archive)?;
    Ok(inspection
        .manifest
        .files
        .into_iter()
        .map(|file| file.archive_path)
        .collect())
}

pub fn verify_archive(archive: &Path) -> Result<ArchiveVerification> {
    match read_archive(archive).and_then(|(envelope, data)| {
        validate_archive(&envelope, &data)?;
        validate_archive_id(&envelope.manifest)?;
        Ok(envelope)
    }) {
        Ok(_) => Ok(ArchiveVerification {
            valid: true,
            issues: Vec::new(),
        }),
        Err(err) => Ok(ArchiveVerification {
            valid: false,
            issues: vec![err.to_string()],
        }),
    }
}

pub fn report_archive(archive: &Path) -> Result<ArchiveReport> {
    let ArchiveInspection {
        manifest,
        sidecar,
        manifest_digest,
        sidecar_digest,
    } = inspect_archive(archive)?;
    let verification = verify_archive(archive)?;
    Ok(ArchiveReport {
        manifest,
        sidecar,
        manifest_digest,
        sidecar_digest,
        verification,
    })
}

pub fn archive_digests(archive: &Path) -> Result<ArchiveDigests> {
    let inspection = inspect_archive(archive)?;
    Ok(ArchiveDigests {
        manifest_digest: inspection.manifest_digest,
        sidecar_digest: inspection.sidecar_digest,
    })
}

pub fn unpack_archive(
    archive: &Path,
    target: &Path,
    selection: Selection,
    overwrite: bool,
) -> Result<ArchiveManifest> {
    let (envelope, data) = read_archive(archive)?;
    validate_archive(&envelope, &data)?;

    ensure_directory_chain_safe(target)?;
    let normalized_target = normalize_directory_path(target)?;
    let target_root = if normalized_target.as_os_str().is_empty() {
        fs::canonicalize(".")?
    } else {
        fs::canonicalize(&normalized_target)?
    };
    if is_filesystem_root(&target_root) {
        return Err(AicxError::UnsafePath(
            "restore target may not be filesystem root".to_string(),
        ));
    }
    let mut selected_any = false;

    for file in &envelope.manifest.files {
        if !selection.matches(&file.archive_path) {
            continue;
        }
        selected_any = true;
        let output_path = safe_output_path(&target_root, &file.archive_path)?;
        ensure_parent_chain_safe(&output_path, &target_root)?;
        ensure_existing_target_safe(&output_path)?;

        let mut restored = Vec::new();
        for chunk_ref in &file.chunk_refs {
            let chunk_index = usize::try_from(*chunk_ref)
                .map_err(|_| AicxError::Validation("chunk reference overflow".to_string()))?;
            let chunk = envelope.manifest.chunks.get(chunk_index).ok_or_else(|| {
                AicxError::Validation(format!("missing chunk reference {chunk_index}"))
            })?;
            let start = usize::try_from(chunk.offset)
                .map_err(|_| AicxError::Validation("chunk offset overflow".to_string()))?;
            let length = usize::try_from(chunk.length)
                .map_err(|_| AicxError::Validation("chunk length overflow".to_string()))?;
            let end = start
                .checked_add(length)
                .ok_or_else(|| AicxError::Validation("chunk range overflow".to_string()))?;
            let compressed = &data[start..end];
            let transformed = decompress(chunk.codec, compressed, chunk.original_size as usize)?;
            if transformed.len() != chunk.original_size as usize {
                return Err(AicxError::Validation(format!(
                    "chunk {} restored size does not match manifest",
                    chunk.chunk_id
                )));
            }
            let restored_chunk =
                reverse_transform(chunk.transform, &transformed, &chunk.transform_metadata);
            let restored_hash = hash_bytes(envelope.manifest.hash_algorithm, &restored_chunk);
            if restored_hash != chunk.hash {
                return Err(AicxError::Validation(format!(
                    "chunk {} hash mismatch during restore",
                    chunk.chunk_id
                )));
            }
            restored.extend_from_slice(&restored_chunk);
        }

        let restored_hash = hash_bytes(envelope.manifest.hash_algorithm, &restored);
        if restored_hash != file.file_hash {
            return Err(AicxError::Validation(format!(
                "file hash mismatch for {}",
                file.archive_path
            )));
        }

        let temp_parent = output_path
            .parent()
            .ok_or_else(|| AicxError::UnsafePath("restore output has no parent".to_string()))?;
        let mut temp_file = new_temp_file(temp_parent, &output_path)?;
        write_temp_file(&mut temp_file, &restored)?;
        apply_restore_permissions(&temp_file, file.executable_hint)?;
        temp_file.as_file_mut().sync_all()?;
        persist_temp_file(temp_file, &output_path, overwrite)?;
    }

    if !selected_any {
        return Err(AicxError::EmptySelection);
    }

    Ok(envelope.manifest)
}

pub fn extract_archive(
    archive: &Path,
    target: &Path,
    selection: Selection,
    overwrite: bool,
) -> Result<ArchiveManifest> {
    unpack_archive(archive, target, selection, overwrite)
}

pub fn compare_profiles(
    inputs: &[PathBuf],
    chunk_size: usize,
) -> Result<Vec<ProfileComparisonRow>> {
    let mut rows = Vec::new();
    for profile in [
        ArchiveProfile::Fast,
        ArchiveProfile::Balanced,
        ArchiveProfile::Max,
        ArchiveProfile::QrMax,
        ArchiveProfile::Agent,
        ArchiveProfile::Secure,
    ] {
        let options = PackOptions {
            chunk_size,
            profile,
            hash_algorithm: HashAlgorithm::Blake3,
        };
        let (envelope, _) = build_from_sources(inputs, &options)?;
        rows.push(ProfileComparisonRow {
            profile,
            stored_size: envelope.manifest.stored_total_size,
            original_size: envelope.manifest.original_total_size,
            ratio: envelope.manifest.compression_ratio,
        });
    }
    Ok(rows)
}
