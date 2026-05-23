use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use aicx_core::{
    inspect_archive, pack_archive, unpack_archive, validate_archive_path, ArchiveEnvelope,
    ArchiveManifest, ArchiveProfile, ArchiveSidecar, ChunkRecord, CodecKind, ExtractionHint,
    FileKind, FileRecord, FileSidecar, HashAlgorithm, PackOptions, Selection, SidecarSummary,
    TransformKind,
};
use tempfile::tempdir;

fn empty_sidecar(manifest: &ArchiveManifest) -> ArchiveSidecar {
    ArchiveSidecar {
        summary: SidecarSummary {
            total_files: manifest.file_count,
            total_size: manifest.original_total_size,
            compressed_size: manifest.stored_total_size,
            compression_ratio: manifest.compression_ratio,
            file_type_counts: BTreeMap::new(),
            codec_usage: BTreeMap::new(),
            transform_usage: BTreeMap::new(),
        },
        file_summaries: vec![FileSidecar {
            archive_path: manifest.files[0].archive_path.clone(),
            file_type: manifest.files[0].file_type,
            size: manifest.files[0].original_size,
            hash: manifest.files[0].file_hash.clone(),
            chunk_count: manifest.files[0].chunk_refs.len() as u64,
            entrypoint_hint: false,
            risk_hints: vec![],
        }],
        entrypoint_candidates: vec![],
        dependency_hints: vec![],
        risk_hints: vec![],
        extraction_map: vec![ExtractionHint {
            archive_path: manifest.files[0].archive_path.clone(),
            chunk_refs: manifest.files[0].chunk_refs.clone(),
        }],
        query_hints: vec![],
        aegisqr_integration_hints: vec![],
    }
}

fn write_archive(path: &Path, envelope: &ArchiveEnvelope, data: &[u8]) {
    let header = serde_cbor::to_vec(envelope).expect("serialize envelope");
    let mut file = File::create(path).expect("create archive");
    file.write_all(b"AICX1").expect("magic");
    file.write_all(&[1]).expect("version");
    file.write_all(&(header.len() as u64).to_be_bytes())
        .expect("header length");
    file.write_all(&header).expect("header");
    file.write_all(data).expect("data");
}

#[test]
fn pack_and_unpack_round_trip() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(input_dir.join("nested")).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");
    fs::write(input_dir.join("nested").join("data.bin"), b"\x00\x01\x02").expect("write bin");

    let archive = temp.path().join("sample.aicx");
    let manifest = pack_archive(
        &[input_dir.clone()],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.file_count, 2);
    assert_eq!(inspect_archive(&archive).expect("inspect").manifest.file_count, 2);

    let restore = temp.path().join("restore");
    let restored_manifest = unpack_archive(&archive, &restore, Selection::default(), false)
        .expect("unpack archive");
    assert_eq!(restored_manifest.file_count, 2);

    assert_eq!(
        fs::read(restore.join("project").join("notes.txt")).expect("read restored"),
        b"hello world\n"
    );
    assert_eq!(
        fs::read(restore.join("project").join("nested").join("data.bin")).expect("read restored"),
        b"\x00\x01\x02"
    );
}

#[test]
fn rejects_traversal_paths() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("traversal.aicx");
    let data = b"payload".to_vec();
    let chunk_hash = blake3::hash(&data).to_hex().to_string();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: aicx_core::CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: chunk_hash,
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "../evil.txt".to_string(),
        original_path: Some("evil.txt".to_string()),
        original_size: data.len() as u64,
        restored_size: data.len() as u64,
        file_hash: blake3::hash(&data).to_hex().to_string(),
        file_type: FileKind::Text,
        detected_language: None,
        transform_applied: TransformKind::Identity,
        chunk_refs: vec![0],
        executable_hint: false,
        risk_hints: vec![],
        metadata_flags: vec![],
    };
    let manifest = ArchiveManifest {
        magic: "AICX1".to_string(),
        version: 1,
        archive_id: "test".to_string(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: data.len() as u64,
        stored_total_size: data.len() as u64,
        compression_ratio: 1.0,
        file_count: 1,
        chunk_count: 1,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash: blake3::hash(&data).to_hex().to_string(),
        files: vec![file],
        chunks: vec![chunk],
    };
    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar: empty_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let restore = temp.path().join("restore");
    let error = unpack_archive(&archive, &restore, Selection::default(), false)
        .expect_err("expected traversal rejection");
    assert!(error.to_string().contains("unsafe archive path"));
}

#[test]
fn rejects_duplicate_archive_paths() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("duplicate.aicx");
    let first = b"first".to_vec();
    let second = b"second".to_vec();

    let chunks = vec![
        ChunkRecord {
            chunk_id: 0,
            file_id: 0,
            chunk_index: 0,
            original_size: first.len() as u64,
            stored_size: first.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&first).to_hex().to_string(),
            offset: 0,
            length: first.len() as u64,
            classification: FileKind::Text,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
        ChunkRecord {
            chunk_id: 1,
            file_id: 1,
            chunk_index: 0,
            original_size: second.len() as u64,
            stored_size: second.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&second).to_hex().to_string(),
            offset: first.len() as u64,
            length: second.len() as u64,
            classification: FileKind::Text,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let archive_path = "project/readme.txt".to_string();
    let files = vec![
        FileRecord {
            archive_path: archive_path.clone(),
            original_path: Some("readme.txt".to_string()),
            original_size: first.len() as u64,
            restored_size: first.len() as u64,
            file_hash: blake3::hash(&first).to_hex().to_string(),
            file_type: FileKind::Text,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![0],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
        FileRecord {
            archive_path,
            original_path: Some("readme-copy.txt".to_string()),
            original_size: second.len() as u64,
            restored_size: second.len() as u64,
            file_hash: blake3::hash(&second).to_hex().to_string(),
            file_type: FileKind::Text,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![1],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
    ];
    let manifest = ArchiveManifest {
        magic: "AICX1".to_string(),
        version: 1,
        archive_id: "test".to_string(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: (first.len() + second.len()) as u64,
        stored_total_size: (first.len() + second.len()) as u64,
        compression_ratio: 1.0,
        file_count: 2,
        chunk_count: 2,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash: blake3::hash(&[first.clone(), second.clone()].concat())
            .to_hex()
            .to_string(),
        files,
        chunks,
    };
    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar: empty_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &[first, second].concat());

    let restore = temp.path().join("restore");
    let error = unpack_archive(&archive, &restore, Selection::default(), false)
        .expect_err("expected duplicate rejection");
    assert!(error.to_string().contains("duplicate archive path"));
}

#[test]
fn validate_archive_path_rejects_absolute_paths() {
    assert!(validate_archive_path("/tmp/evil").is_err());
}
