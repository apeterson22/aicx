use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use blake3::Hasher;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::chunking::fixed_size_chunks;
use crate::classify::{classify, detect_language};
use crate::compression::{decompress, select_codec};
use crate::error::{AicxError, Result};
use crate::model::*;
use crate::pathing::{archive_path_to_string, ensure_parent_chain_safe, safe_output_path, validate_archive_path};
use crate::sidecar::build_sidecar;
use crate::transform::{apply_transform, reverse_transform};

const MAGIC: &[u8; 5] = b"AICX1";
const VERSION: u8 = 1;
const CREATED_BY: &str = "aicx-rust";

#[derive(Debug, Clone)]
struct SourceFile {
    source_path: PathBuf,
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

fn archive_path_from_root(root_name: &str, relative_path: &Path) -> Result<String> {
    let mut combined = PathBuf::from(root_name);
    combined.push(relative_path);
    let archive_path = archive_path_to_string(&combined);
    validate_archive_path(&archive_path)?;
    Ok(archive_path)
}

fn original_path_if_safe(path: &Path) -> Option<String> {
    if path.is_relative() {
        let raw = path.to_string_lossy().to_string();
        if !raw.is_empty() {
            return Some(raw);
        }
    }
    None
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
    let mut files = Vec::new();

    for input in inputs {
        let metadata = fs::symlink_metadata(input)?;
        if metadata.file_type().is_symlink() {
            return Err(AicxError::UnsafePath(format!(
                "refusing to pack symlink source: {}",
                input.display()
            )));
        }
        if metadata.is_dir() {
            let root_name = input
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    AicxError::UnsafePath(format!(
                        "directory input has no usable name: {}",
                        input.display()
                    ))
                })?
                .to_string();
            for entry in WalkDir::new(input).follow_links(false) {
                let entry = entry.map_err(|err| AicxError::Validation(err.to_string()))?;
                if entry.path() == input {
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
                    let relative = entry
                        .path()
                        .strip_prefix(input)
                        .map_err(|_| AicxError::Validation("failed to compute relative path".to_string()))?;
                    let archive_path = archive_path_from_root(&root_name, relative)?;
                    let bytes = fs::read(entry.path())?;
                    files.push(SourceFile {
                        source_path: entry.path().to_path_buf(),
                        archive_path,
                        original_path: original_path_if_safe(entry.path()),
                        executable_hint: is_executable_hint(entry.path(), &bytes),
                        bytes,
                    });
                }
            }
        } else if metadata.is_file() {
            let root_name = input
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    AicxError::UnsafePath(format!("input path has no usable name: {}", input.display()))
                })?
                .to_string();
            let archive_path = archive_path_from_root(&root_name, Path::new(""))?;
            let bytes = fs::read(input)?;
            files.push(SourceFile {
                source_path: input.clone(),
                archive_path,
                original_path: original_path_if_safe(input),
                executable_hint: is_executable_hint(input, &bytes),
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

fn build_from_sources(inputs: &[PathBuf], options: &PackOptions) -> Result<(ArchiveEnvelope, Vec<u8>)> {
    if options.chunk_size == 0 {
        return Err(AicxError::Validation("chunk size must be positive".to_string()));
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
            let classification = classify(&source.archive_path, chunk);
            let (transformed, transform, transform_metadata) = apply_transform(classification, chunk);
            let (codec, codec_level, compressed) = select_codec(options.profile, classification, &transformed)?;
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
    let manifest = ArchiveManifest {
        magic: "AICX1".to_string(),
        version: VERSION as u32,
        archive_id: blake3::hash(
            format!(
                "{}:{}:{}:{}",
                CREATED_BY, options.profile, file_records.len(), data_hash
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string(),
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
    let sidecar = build_sidecar(&manifest);
    Ok((ArchiveEnvelope { manifest, sidecar }, chunk_data))
}

fn write_archive(output: &Path, envelope: &ArchiveEnvelope, data: &[u8]) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest_bytes = serde_cbor::to_vec(envelope)?;
    let mut writer = BufWriter::new(File::create(output)?);
    writer.write_all(MAGIC)?;
    writer.write_all(&[VERSION])?;
    writer.write_all(&(manifest_bytes.len() as u64).to_be_bytes())?;
    writer.write_all(&manifest_bytes)?;
    writer.write_all(data)?;
    writer.flush()?;
    Ok(())
}

fn read_archive(archive: &Path) -> Result<(ArchiveEnvelope, Vec<u8>)> {
    let file = File::open(archive)?;
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; 5];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(AicxError::Validation("invalid AICX magic bytes".to_string()));
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
    let mut header_bytes = vec![0u8; header_len as usize];
    reader.read_exact(&mut header_bytes)?;
    let envelope: ArchiveEnvelope = serde_cbor::from_slice(&header_bytes)?;
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    Ok((envelope, data))
}

fn validate_archive(envelope: &ArchiveEnvelope, data: &[u8]) -> Result<()> {
    let manifest = &envelope.manifest;
    if manifest.magic != "AICX1" {
        return Err(AicxError::Validation("manifest magic mismatch".to_string()));
    }
    if manifest.version != VERSION as u32 {
        return Err(AicxError::Validation(format!(
            "unsupported manifest version: {}",
            manifest.version
        )));
    }
    if manifest.chunk_size == 0 {
        return Err(AicxError::Validation("chunk size must be positive".to_string()));
    }
    if manifest.file_count as usize != manifest.files.len() {
        return Err(AicxError::Validation("file count does not match manifest".to_string()));
    }
    if manifest.chunk_count as usize != manifest.chunks.len() {
        return Err(AicxError::Validation("chunk count does not match manifest".to_string()));
    }
    if manifest.stored_total_size as usize != data.len() {
        return Err(AicxError::Validation(
            "stored size does not match chunk data length".to_string(),
        ));
    }
    let actual_data_hash = hash_bytes(manifest.hash_algorithm, data);
    if actual_data_hash != manifest.data_hash {
        return Err(AicxError::Validation(
            "chunk data hash does not match manifest".to_string(),
        ));
    }

    for (idx, chunk) in manifest.chunks.iter().enumerate() {
        if chunk.chunk_id != idx as u64 {
            return Err(AicxError::Validation(format!(
                "chunk {idx} has unexpected chunk_id {}",
                chunk.chunk_id
            )));
        }
        if chunk.file_id as usize >= manifest.files.len() {
            return Err(AicxError::Validation(format!(
                "chunk {idx} references missing file {}",
                chunk.file_id
            )));
        }
        let offset = usize::try_from(chunk.offset)
            .map_err(|_| AicxError::Validation(format!("chunk {idx} offset overflows host usize")))?;
        let length = usize::try_from(chunk.length)
            .map_err(|_| AicxError::Validation(format!("chunk {idx} length overflows host usize")))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| AicxError::Validation(format!("chunk {idx} range overflows")))?;
        if end > data.len() {
            return Err(AicxError::Validation(format!(
                "chunk {idx} exceeds available chunk data"
            )));
        }
    }

    let mut seen_archive_paths = std::collections::BTreeSet::new();
    for (file_idx, file) in manifest.files.iter().enumerate() {
        validate_archive_path(&file.archive_path)?;
        if !seen_archive_paths.insert(file.archive_path.clone()) {
            return Err(AicxError::Validation(format!(
                "duplicate archive path in manifest: {}",
                file.archive_path
            )));
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
        for (position, chunk_ref) in file.chunk_refs.iter().enumerate() {
            let chunk_index = usize::try_from(*chunk_ref).map_err(|_| {
                AicxError::Validation(format!(
                    "file {file_idx} chunk reference {position} overflows host usize"
                ))
            })?;
            let chunk = manifest.chunks.get(chunk_index).ok_or_else(|| {
                AicxError::Validation(format!("file {file_idx} references missing chunk {chunk_index}"))
            })?;
            reconstructed_size = reconstructed_size
                .checked_add(chunk.original_size)
                .ok_or_else(|| {
                    AicxError::Validation(format!("file {file_idx} size overflows validation"))
                })?;
        }
        if reconstructed_size != file.original_size {
            return Err(AicxError::Validation(format!(
                "file {file_idx} size mismatch: manifest={} reconstructed={}",
                file.original_size, reconstructed_size
            )));
        }
    }

    Ok(())
}

pub fn pack_archive(inputs: &[PathBuf], output: &Path, options: PackOptions) -> Result<ArchiveManifest> {
    let (envelope, data) = build_from_sources(inputs, &options)?;
    write_archive(output, &envelope, &data)?;
    Ok(envelope.manifest)
}

pub fn inspect_archive(archive: &Path) -> Result<ArchiveInspection> {
    let (envelope, data) = read_archive(archive)?;
    validate_archive(&envelope, &data)?;
    Ok(ArchiveInspection {
        manifest: envelope.manifest,
        sidecar: envelope.sidecar,
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
    match read_archive(archive).and_then(|(envelope, data)| validate_archive(&envelope, &data).map(|_| envelope)) {
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
    let inspection = inspect_archive(archive)?;
    let verification = verify_archive(archive)?;
    Ok(ArchiveReport {
        manifest: inspection.manifest,
        sidecar: inspection.sidecar,
        verification,
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

    if !target.exists() {
        fs::create_dir_all(target)?;
    }
    let target_root = fs::canonicalize(target)?;
    let mut selected_any = false;

    for file in &envelope.manifest.files {
        if !selection.matches(&file.archive_path) {
            continue;
        }
        selected_any = true;
        let output_path = safe_output_path(&target_root, &file.archive_path)?;
        ensure_parent_chain_safe(&output_path, &target_root)?;
        if output_path.exists() {
            let metadata = fs::symlink_metadata(&output_path)?;
            if metadata.file_type().is_symlink() {
                return Err(AicxError::UnsafePath(format!(
                    "refusing to write through symlink: {}",
                    output_path.display()
                )));
            }
            if !overwrite {
                return Err(AicxError::OverwriteDenied(output_path.display().to_string()));
            }
        }

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
            let transformed = decompress(chunk.codec, compressed)?;
            let restored_chunk = reverse_transform(chunk.transform, &transformed, &chunk.transform_metadata);
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

        let mut writer = if overwrite {
            BufWriter::new(
                OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&output_path)?,
            )
        } else {
            BufWriter::new(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&output_path)?,
            )
        };
        writer.write_all(&restored)?;
        writer.flush()?;
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

pub fn compare_profiles(inputs: &[PathBuf], chunk_size: usize) -> Result<Vec<ProfileComparisonRow>> {
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
