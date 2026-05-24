use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use aicx_core::{
    archive_digests, build_sidecar, inspect_archive, manifest_digest, pack_archive, report_archive,
    sidecar_digest, unpack_archive, validate_archive_path, ArchiveEnvelope, ArchiveManifest,
    ArchiveProfile, ArchiveSidecar, ChunkRecord, CodecKind, ExtractionHint, FileKind, FileRecord,
    FileSidecar, HashAlgorithm, PackOptions, Selection, SidecarSummary, TransformKind,
};
use tempfile::tempdir;

fn deterministic_sidecar(manifest: &ArchiveManifest) -> ArchiveSidecar {
    build_sidecar(manifest)
}

fn write_archive(path: &Path, envelope: &ArchiveEnvelope, data: &[u8]) {
    let header = serde_cbor::to_vec(envelope).expect("serialize envelope");
    let mut file = File::create(path).expect("create archive");
    file.write_all(b"AICX2").expect("magic");
    file.write_all(&[2]).expect("version");
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
        std::slice::from_ref(&input_dir),
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.file_count, 2);
    assert_eq!(
        inspect_archive(&archive)
            .expect("inspect")
            .manifest
            .file_count,
        2
    );

    let restore = temp.path().join("restore");
    let restored_manifest =
        unpack_archive(&archive, &restore, Selection::default(), false).expect("unpack archive");
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
fn archive_id_changes_when_manifest_metadata_changes() {
    let temp = tempdir().expect("tempdir");

    let first_dir = temp.path().join("alpha");
    let second_dir = temp.path().join("beta");
    fs::create_dir_all(&first_dir).expect("create alpha");
    fs::create_dir_all(&second_dir).expect("create beta");
    fs::write(first_dir.join("notes.txt"), b"same payload\n").expect("write alpha");
    fs::write(second_dir.join("notes.txt"), b"same payload\n").expect("write beta");

    let first_archive = temp.path().join("alpha.aicx");
    let second_archive = temp.path().join("beta.aicx");

    let first_manifest = pack_archive(
        &[first_dir],
        &first_archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack first archive");
    let second_manifest = pack_archive(
        &[second_dir],
        &second_archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack second archive");

    assert_ne!(first_manifest.archive_id, second_manifest.archive_id);
}

#[test]
fn pack_refuses_to_overwrite_existing_archive() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("sample.aicx");
    pack_archive(
        std::slice::from_ref(&input_dir),
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let error = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected overwrite denial");
    assert!(error.to_string().contains("overwrite denied"));
}

#[cfg(unix)]
#[test]
fn pack_rejects_symlinked_output_parent() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let real_output_dir = temp.path().join("real-output");
    fs::create_dir_all(&real_output_dir).expect("create real output");
    let symlink_output_dir = temp.path().join("linked-output");
    symlink(&real_output_dir, &symlink_output_dir).expect("create symlink");

    let archive = symlink_output_dir.join("sample.aicx");
    let error = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected symlink rejection");
    assert!(error.to_string().contains("symlink escape rejected"));
}

#[cfg(unix)]
#[test]
fn pack_ignores_intermediate_symlink_traversal_in_output_path() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let safe_dir = temp.path().join("safe");
    let escape_dir = temp.path().join("escape");
    fs::create_dir_all(&safe_dir).expect("create safe");
    fs::create_dir_all(&escape_dir).expect("create escape");
    symlink(&escape_dir, safe_dir.join("sub")).expect("create symlink");

    let archive = safe_dir.join("sub").join("..").join("archive.aicx");
    let manifest = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.file_count, 1);
    assert!(safe_dir.join("archive.aicx").exists());
    assert!(!temp.path().join("archive.aicx").exists());
}

#[test]
fn pack_rejects_dotdot_output_filename() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("nested").join("..");
    let error = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected unsafe file name rejection");
    assert!(error.to_string().contains("unsafe archive path"));
    assert!(!temp.path().join("nested").exists());
}

#[cfg(unix)]
#[test]
fn pack_rejects_non_utf8_output_filename() {
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let mut bad_name = temp.path().to_path_buf();
    bad_name.push(std::ffi::OsString::from_vec(b"bad-\xff.aicx".to_vec()));
    let error = pack_archive(
        &[input_dir],
        &bad_name,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected non-utf8 filename rejection");
    assert!(error.to_string().contains("filename must be UTF-8"));
}

#[cfg(unix)]
#[test]
fn pack_rejects_filesystem_root_output() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let error = pack_archive(
        &[input_dir],
        Path::new("/sample.aicx"),
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected filesystem root rejection");
    assert!(error.to_string().contains("filesystem root"));
}

#[cfg(unix)]
#[test]
fn pack_rejects_existing_hard_link_output_target() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let outside_link = temp.path().join("shared.aicx");
    fs::write(&outside_link, b"shared\n").expect("write shared");
    let archive = temp.path().join("archive.aicx");
    fs::hard_link(&outside_link, &archive).expect("create hard link");

    let error = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected hard link rejection");
    assert!(error.to_string().contains("hard-linked file"));
    assert_eq!(fs::read(&outside_link).expect("read shared"), b"shared\n");
}

#[test]
fn pack_supports_parent_dir_in_output_path() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("nested").join("..").join("sample.aicx");
    let manifest = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.file_count, 1);
    assert!(temp.path().join("sample.aicx").exists());
}

#[test]
fn pack_supports_relative_parent_dir_output_path() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    struct CwdGuard(PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    let cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(cwd);
    std::env::set_current_dir(temp.path()).expect("set current dir");
    let result = pack_archive(
        &[input_dir],
        Path::new("nested/../sample.aicx"),
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    );

    let manifest = result.expect("pack archive");
    assert_eq!(manifest.file_count, 1);
    assert!(temp.path().join("sample.aicx").exists());
    assert!(!temp.path().join("nested").exists());
}

#[test]
fn pack_normalizes_original_path_metadata() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    struct CwdGuard(PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    let cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(cwd);
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let manifest = pack_archive(
        &[PathBuf::from("project/../project/notes.txt")],
        &PathBuf::from("archive.aicx"),
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.files.len(), 1);
    assert_eq!(
        manifest.files[0].original_path.as_deref(),
        Some("project/notes.txt")
    );
}

#[cfg(unix)]
#[test]
fn pack_rejects_non_utf8_input_path() {
    use std::os::unix::ffi::OsStringExt;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    let bad_path = input_dir.join(std::ffi::OsString::from_vec(b"bad-\xff.txt".to_vec()));
    fs::write(&bad_path, b"hello world\n").expect("write text");

    let archive = temp.path().join("archive.aicx");
    let error = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect_err("expected non-utf8 input path rejection");
    assert!(error.to_string().contains("UTF-8"));
}

#[test]
fn pack_preserves_directory_source_metadata() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(input_dir.join("nested")).expect("create input");
    fs::write(input_dir.join("nested").join("data.bin"), b"\x00\x01\x02").expect("write bin");

    let archive = temp.path().join("archive.aicx");
    let manifest = pack_archive(
        std::slice::from_ref(&input_dir),
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let nested = manifest
        .files
        .iter()
        .find(|file| file.archive_path.ends_with("data.bin"))
        .expect("nested file");
    assert_eq!(
        nested.original_path.as_deref(),
        Some("project/nested/data.bin")
    );
}

#[test]
fn pack_preserves_absolute_file_source_metadata() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("archive.aicx");
    let manifest = pack_archive(
        &[input_dir.join("notes.txt")],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.files.len(), 1);
    assert_eq!(
        manifest.files[0].original_path.as_deref(),
        Some("notes.txt")
    );
}

#[test]
fn pack_preserves_relative_file_source_metadata() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    struct CwdGuard(PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    let cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(cwd);
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let archive = temp.path().join("archive.aicx");
    let manifest = pack_archive(
        &[PathBuf::from("project/notes.txt")],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert_eq!(manifest.files.len(), 1);
    assert_eq!(
        manifest.files[0].original_path.as_deref(),
        Some("project/notes.txt")
    );
}

#[test]
fn pack_accepts_current_directory_input() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    struct CwdGuard(PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    let cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(cwd);
    std::env::set_current_dir(temp.path()).expect("set current dir");

    let archive = temp.path().join("archive.aicx");
    let manifest = pack_archive(
        &[PathBuf::from(".")],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let root_name = temp
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let expected_archive_path = format!("{root_name}/project/notes.txt");
    assert_eq!(manifest.file_count, 1);
    assert_eq!(manifest.files[0].archive_path, expected_archive_path);
    assert_eq!(
        manifest.files[0].original_path.as_deref(),
        Some(expected_archive_path.as_str())
    );
}

#[test]
fn rejects_noncanonical_original_path_metadata() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("archive.aicx");
    let mut manifest = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    manifest.files[0].original_path = Some("project/./notes.txt".to_string());

    #[derive(serde::Serialize)]
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

    let seed = ArchiveIdentitySeed {
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
    };
    manifest.archive_id = blake3::hash(
        serde_json::to_string(&seed)
            .expect("serialize seed")
            .as_bytes(),
    )
    .to_hex()
    .to_string();

    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, b"hello world\n");

    let error = inspect_archive(&archive).expect_err("expected original path validation failure");
    assert!(error
        .to_string()
        .contains("non-canonical original path metadata"));
}

#[cfg(unix)]
#[test]
fn unpack_restores_executable_hint() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    let script = input_dir.join("run.sh");
    fs::write(&script, b"#!/bin/sh\necho hello\n").expect("write script");
    let mut perms = fs::metadata(&script).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).expect("chmod");

    let archive = temp.path().join("script.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let inspection = inspect_archive(&archive).expect("inspect archive");
    assert!(inspection
        .manifest
        .files
        .iter()
        .any(|file| file.archive_path.ends_with("run.sh") && file.executable_hint));

    let restore = temp.path().join("restore");
    unpack_archive(&archive, &restore, Selection::default(), false).expect("unpack archive");
    let restored = restore.join("project").join("run.sh");
    let restored_mode = fs::metadata(&restored)
        .expect("restored metadata")
        .permissions()
        .mode();
    assert!(restored_mode & 0o111 != 0);
}

#[cfg(unix)]
#[test]
fn unpack_rejects_symlinked_target_root() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("sample.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let actual_target = temp.path().join("actual-target");
    fs::create_dir_all(&actual_target).expect("create target");
    let symlink_target = temp.path().join("target-link");
    symlink(&actual_target, &symlink_target).expect("create symlink");

    let error = unpack_archive(&archive, &symlink_target, Selection::default(), false)
        .expect_err("expected target root rejection");
    assert!(error.to_string().contains("symlink escape rejected"));
}

#[cfg(unix)]
#[test]
fn unpack_rejects_filesystem_root_target() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("sample.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let error = unpack_archive(&archive, Path::new("/"), Selection::default(), false)
        .expect_err("expected filesystem root target rejection");
    assert!(error.to_string().contains("filesystem root"));
}

#[cfg(unix)]
#[test]
fn unpack_rejects_existing_non_regular_output_target_on_overwrite() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("sample.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let target_root = temp.path().join("restore");
    let output_path = target_root.join("project").join("notes.txt");
    fs::create_dir_all(output_path.parent().expect("parent")).expect("create output parent");

    let c_path = CString::new(output_path.as_os_str().as_bytes()).expect("c path");
    let fifo_result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    assert_eq!(fifo_result, 0, "mkfifo should succeed");

    let error = unpack_archive(&archive, &target_root, Selection::default(), true)
        .expect_err("expected non-regular file rejection");
    assert!(error.to_string().contains("non-regular file"));
}

#[cfg(unix)]
#[test]
fn unpack_rejects_existing_hard_link_output_target_on_overwrite() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("sample.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let target_root = temp.path().join("restore");
    let output_path = target_root.join("project").join("notes.txt");
    fs::create_dir_all(output_path.parent().expect("parent")).expect("create output parent");

    let outside_link = temp.path().join("shared.txt");
    fs::write(&outside_link, b"shared\n").expect("write shared");
    fs::hard_link(&outside_link, &output_path).expect("create hard link");

    let error = unpack_archive(&archive, &target_root, Selection::default(), true)
        .expect_err("expected hard link rejection");
    assert!(error.to_string().contains("hard-linked file"));
    assert_eq!(fs::read(&outside_link).expect("read shared"), b"shared\n");
}

#[test]
fn unpack_keeps_existing_file_intact_when_restore_fails() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("corrupt.aicx");
    let first = b"first".to_vec();
    let second = b"second".to_vec();
    let data = [first.clone(), second.clone()].concat();

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
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
        ChunkRecord {
            chunk_id: 1,
            file_id: 0,
            chunk_index: 1,
            original_size: second.len() as u64,
            stored_size: second.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: "incorrect".to_string(),
            offset: first.len() as u64,
            length: second.len() as u64,
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let file = FileRecord {
        archive_path: "project/notes.txt".to_string(),
        original_path: Some("notes.txt".to_string()),
        original_size: data.len() as u64,
        restored_size: data.len() as u64,
        file_hash: blake3::hash(&data).to_hex().to_string(),
        file_type: FileKind::Logs,
        detected_language: None,
        transform_applied: TransformKind::Identity,
        chunk_refs: vec![0, 1],
        executable_hint: false,
        risk_hints: vec![],
        metadata_flags: vec![],
    };
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: "test".to_string(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: data.len() as u64,
        stored_total_size: data.len() as u64,
        compression_ratio: 1.0,
        file_count: 1,
        chunk_count: 2,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash: blake3::hash(&data).to_hex().to_string(),
        files: vec![file],
        chunks,
    };
    manifest.archive_id = "test".to_string();
    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let target = temp.path().join("restore");
    let existing_output = target.join("project").join("notes.txt");
    fs::create_dir_all(existing_output.parent().expect("parent")).expect("create output parent");
    fs::write(&existing_output, b"original contents").expect("seed existing file");

    let error = unpack_archive(&archive, &target, Selection::default(), true)
        .expect_err("expected chunk verification failure");
    assert!(error
        .to_string()
        .contains("chunk 1 hash mismatch during restore"));
    assert_eq!(
        fs::read(&existing_output).expect("read output"),
        b"original contents"
    );
}

#[test]
fn rejects_unsorted_manifest_files() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("unsorted.aicx");
    let first = b"first".to_vec();
    let second = b"second".to_vec();
    let data = [second.clone(), first.clone()].concat();

    let chunks = vec![
        ChunkRecord {
            chunk_id: 0,
            file_id: 0,
            chunk_index: 0,
            original_size: second.len() as u64,
            stored_size: second.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&second).to_hex().to_string(),
            offset: 0,
            length: second.len() as u64,
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
        ChunkRecord {
            chunk_id: 1,
            file_id: 1,
            chunk_index: 0,
            original_size: first.len() as u64,
            stored_size: first.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&first).to_hex().to_string(),
            offset: second.len() as u64,
            length: first.len() as u64,
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let files = vec![
        FileRecord {
            archive_path: "project/b.txt".to_string(),
            original_path: Some("b.txt".to_string()),
            original_size: second.len() as u64,
            restored_size: second.len() as u64,
            file_hash: blake3::hash(&second).to_hex().to_string(),
            file_type: FileKind::Logs,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![0],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
        FileRecord {
            archive_path: "project/a.txt".to_string(),
            original_path: Some("a.txt".to_string()),
            original_size: first.len() as u64,
            restored_size: first.len() as u64,
            file_hash: blake3::hash(&first).to_hex().to_string(),
            file_type: FileKind::Logs,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![1],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
    ];
    let data_hash = blake3::hash(&data).to_hex().to_string();
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: String::new(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: data.len() as u64,
        stored_total_size: data.len() as u64,
        compression_ratio: 1.0,
        file_count: 2,
        chunk_count: 2,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash,
        files,
        chunks,
    };

    #[derive(serde::Serialize)]
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

    let seed = ArchiveIdentitySeed {
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
    };
    manifest.archive_id = blake3::hash(
        serde_json::to_string(&seed)
            .expect("serialize seed")
            .as_bytes(),
    )
    .to_hex()
    .to_string();

    let sidecar = ArchiveSidecar {
        summary: SidecarSummary {
            total_files: manifest.file_count,
            total_size: manifest.original_total_size,
            compressed_size: manifest.stored_total_size,
            compression_ratio: manifest.compression_ratio,
            file_type_counts: BTreeMap::from([(FileKind::Logs.to_string(), 2)]),
            codec_usage: BTreeMap::from([(CodecKind::None.to_string(), 2)]),
            transform_usage: BTreeMap::from([(TransformKind::Identity.to_string(), 2)]),
        },
        file_summaries: vec![
            FileSidecar {
                archive_path: "project/b.txt".to_string(),
                file_type: FileKind::Logs,
                size: second.len() as u64,
                hash: blake3::hash(&second).to_hex().to_string(),
                chunk_count: 1,
                entrypoint_hint: false,
                risk_hints: vec![],
            },
            FileSidecar {
                archive_path: "project/a.txt".to_string(),
                file_type: FileKind::Logs,
                size: first.len() as u64,
                hash: blake3::hash(&first).to_hex().to_string(),
                chunk_count: 1,
                entrypoint_hint: false,
                risk_hints: vec![],
            },
        ],
        entrypoint_candidates: vec!["no obvious entrypoint detected".to_string()],
        dependency_hints: vec!["no dependency manifest detected".to_string()],
        risk_hints: vec![],
        extraction_map: vec![
            ExtractionHint {
                archive_path: "project/b.txt".to_string(),
                chunk_refs: vec![0],
            },
            ExtractionHint {
                archive_path: "project/a.txt".to_string(),
                chunk_refs: vec![1],
            },
        ],
        query_hints: vec!["type:logs".to_string(), "type:logs".to_string()],
        aegisqr_integration_hints: vec![
            "use qr-max for smallest transport bundles".to_string(),
            "wrap manifest and sidecar hashes before encryption".to_string(),
        ],
    };

    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected canonical ordering rejection");
    assert!(error
        .to_string()
        .contains("files are not sorted by archive path"));
}

#[test]
fn rejects_unsorted_manifest_chunks() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("unsorted-chunks.aicx");
    let first = b"first".to_vec();
    let second = b"second".to_vec();
    let data = [second.clone(), first.clone()].concat();

    let chunks = vec![
        ChunkRecord {
            chunk_id: 0,
            file_id: 1,
            chunk_index: 0,
            original_size: second.len() as u64,
            stored_size: second.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&second).to_hex().to_string(),
            offset: 0,
            length: second.len() as u64,
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
        ChunkRecord {
            chunk_id: 1,
            file_id: 0,
            chunk_index: 0,
            original_size: first.len() as u64,
            stored_size: first.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&first).to_hex().to_string(),
            offset: second.len() as u64,
            length: first.len() as u64,
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let files = vec![
        FileRecord {
            archive_path: "project/a.txt".to_string(),
            original_path: Some("a.txt".to_string()),
            original_size: first.len() as u64,
            restored_size: first.len() as u64,
            file_hash: blake3::hash(&first).to_hex().to_string(),
            file_type: FileKind::Logs,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![1],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
        FileRecord {
            archive_path: "project/b.txt".to_string(),
            original_path: Some("b.txt".to_string()),
            original_size: second.len() as u64,
            restored_size: second.len() as u64,
            file_hash: blake3::hash(&second).to_hex().to_string(),
            file_type: FileKind::Logs,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![0],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
    ];
    let data_hash = blake3::hash(&data).to_hex().to_string();
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: String::new(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: data.len() as u64,
        stored_total_size: data.len() as u64,
        compression_ratio: 1.0,
        file_count: 2,
        chunk_count: 2,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash,
        files,
        chunks,
    };

    #[derive(serde::Serialize)]
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

    let seed = ArchiveIdentitySeed {
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
    };
    manifest.archive_id = blake3::hash(
        serde_json::to_string(&seed)
            .expect("serialize seed")
            .as_bytes(),
    )
    .to_hex()
    .to_string();

    let sidecar = ArchiveSidecar {
        summary: SidecarSummary {
            total_files: manifest.file_count,
            total_size: manifest.original_total_size,
            compressed_size: manifest.stored_total_size,
            compression_ratio: manifest.compression_ratio,
            file_type_counts: BTreeMap::from([(FileKind::Logs.to_string(), 2)]),
            codec_usage: BTreeMap::from([(CodecKind::None.to_string(), 2)]),
            transform_usage: BTreeMap::from([(TransformKind::Identity.to_string(), 2)]),
        },
        file_summaries: vec![
            FileSidecar {
                archive_path: "project/a.txt".to_string(),
                file_type: FileKind::Logs,
                size: first.len() as u64,
                hash: blake3::hash(&first).to_hex().to_string(),
                chunk_count: 1,
                entrypoint_hint: false,
                risk_hints: vec![],
            },
            FileSidecar {
                archive_path: "project/b.txt".to_string(),
                file_type: FileKind::Logs,
                size: second.len() as u64,
                hash: blake3::hash(&second).to_hex().to_string(),
                chunk_count: 1,
                entrypoint_hint: false,
                risk_hints: vec![],
            },
        ],
        entrypoint_candidates: vec!["no obvious entrypoint detected".to_string()],
        dependency_hints: vec!["no dependency manifest detected".to_string()],
        risk_hints: vec![],
        extraction_map: vec![
            ExtractionHint {
                archive_path: "project/a.txt".to_string(),
                chunk_refs: vec![1],
            },
            ExtractionHint {
                archive_path: "project/b.txt".to_string(),
                chunk_refs: vec![0],
            },
        ],
        query_hints: vec!["type:logs".to_string(), "type:logs".to_string()],
        aegisqr_integration_hints: vec![
            "use qr-max for smallest transport bundles".to_string(),
            "wrap manifest and sidecar hashes before encryption".to_string(),
        ],
    };

    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected canonical chunk ordering rejection");
    assert!(error
        .to_string()
        .contains("chunks are not sorted by file id"));
}

#[test]
fn rejects_cross_file_chunk_references() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("cross-file-chunks.aicx");
    let first = b"first".to_vec();
    let second = b"second".to_vec();
    let data = [first.clone(), second.clone()].concat();

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
            classification: FileKind::Logs,
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
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let files = vec![
        FileRecord {
            archive_path: "project/a.txt".to_string(),
            original_path: Some("a.txt".to_string()),
            original_size: second.len() as u64,
            restored_size: second.len() as u64,
            file_hash: blake3::hash(&second).to_hex().to_string(),
            file_type: FileKind::Logs,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![1],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
        FileRecord {
            archive_path: "project/b.txt".to_string(),
            original_path: Some("b.txt".to_string()),
            original_size: first.len() as u64,
            restored_size: first.len() as u64,
            file_hash: blake3::hash(&first).to_hex().to_string(),
            file_type: FileKind::Logs,
            detected_language: None,
            transform_applied: TransformKind::Identity,
            chunk_refs: vec![0],
            executable_hint: false,
            risk_hints: vec![],
            metadata_flags: vec![],
        },
    ];
    let data_hash = blake3::hash(&data).to_hex().to_string();
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: String::new(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: data.len() as u64,
        stored_total_size: data.len() as u64,
        compression_ratio: 1.0,
        file_count: 2,
        chunk_count: 2,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash,
        files,
        chunks,
    };

    #[derive(serde::Serialize)]
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

    let seed = ArchiveIdentitySeed {
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
    };
    manifest.archive_id = blake3::hash(
        serde_json::to_string(&seed)
            .expect("serialize seed")
            .as_bytes(),
    )
    .to_hex()
    .to_string();

    let sidecar = ArchiveSidecar {
        summary: SidecarSummary {
            total_files: manifest.file_count,
            total_size: manifest.original_total_size,
            compressed_size: manifest.stored_total_size,
            compression_ratio: manifest.compression_ratio,
            file_type_counts: BTreeMap::from([(FileKind::Logs.to_string(), 2)]),
            codec_usage: BTreeMap::from([(CodecKind::None.to_string(), 2)]),
            transform_usage: BTreeMap::from([(TransformKind::Identity.to_string(), 2)]),
        },
        file_summaries: vec![
            FileSidecar {
                archive_path: "project/a.txt".to_string(),
                file_type: FileKind::Logs,
                size: second.len() as u64,
                hash: blake3::hash(&second).to_hex().to_string(),
                chunk_count: 1,
                entrypoint_hint: false,
                risk_hints: vec![],
            },
            FileSidecar {
                archive_path: "project/b.txt".to_string(),
                file_type: FileKind::Logs,
                size: first.len() as u64,
                hash: blake3::hash(&first).to_hex().to_string(),
                chunk_count: 1,
                entrypoint_hint: false,
                risk_hints: vec![],
            },
        ],
        entrypoint_candidates: vec!["no obvious entrypoint detected".to_string()],
        dependency_hints: vec!["no dependency manifest detected".to_string()],
        risk_hints: vec![],
        extraction_map: vec![
            ExtractionHint {
                archive_path: "project/a.txt".to_string(),
                chunk_refs: vec![1],
            },
            ExtractionHint {
                archive_path: "project/b.txt".to_string(),
                chunk_refs: vec![0],
            },
        ],
        query_hints: vec!["type:logs".to_string(), "type:logs".to_string()],
        aegisqr_integration_hints: vec![
            "use qr-max for smallest transport bundles".to_string(),
            "wrap manifest and sidecar hashes before encryption".to_string(),
        ],
    };

    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected cross-file reference rejection");
    assert!(error
        .to_string()
        .contains("chunk ownership does not match file"));
}

#[test]
fn rejects_orphaned_chunks() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("orphan-chunks.aicx");
    let first = b"first".to_vec();
    let second = b"second".to_vec();
    let data = [first.clone(), second.clone()].concat();

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
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
        ChunkRecord {
            chunk_id: 1,
            file_id: 0,
            chunk_index: 1,
            original_size: second.len() as u64,
            stored_size: second.len() as u64,
            codec: CodecKind::None,
            codec_level: 0,
            transform: TransformKind::Identity,
            hash: blake3::hash(&second).to_hex().to_string(),
            offset: first.len() as u64,
            length: second.len() as u64,
            classification: FileKind::Logs,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let files = vec![FileRecord {
        archive_path: "project/a.txt".to_string(),
        original_path: Some("a.txt".to_string()),
        original_size: first.len() as u64,
        restored_size: first.len() as u64,
        file_hash: blake3::hash(&first).to_hex().to_string(),
        file_type: FileKind::Logs,
        detected_language: None,
        transform_applied: TransformKind::Identity,
        chunk_refs: vec![0],
        executable_hint: false,
        risk_hints: vec![],
        metadata_flags: vec![],
    }];
    let data_hash = blake3::hash(&data).to_hex().to_string();
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: String::new(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: first.len() as u64,
        stored_total_size: data.len() as u64,
        compression_ratio: 1.0,
        file_count: 1,
        chunk_count: 2,
        hash_algorithm: HashAlgorithm::Blake3,
        data_hash,
        files,
        chunks,
    };

    #[derive(serde::Serialize)]
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

    let seed = ArchiveIdentitySeed {
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
    };
    manifest.archive_id = blake3::hash(
        serde_json::to_string(&seed)
            .expect("serialize seed")
            .as_bytes(),
    )
    .to_hex()
    .to_string();

    let sidecar = ArchiveSidecar {
        summary: SidecarSummary {
            total_files: manifest.file_count,
            total_size: manifest.original_total_size,
            compressed_size: manifest.stored_total_size,
            compression_ratio: manifest.compression_ratio,
            file_type_counts: BTreeMap::from([(FileKind::Logs.to_string(), 1)]),
            codec_usage: BTreeMap::from([(CodecKind::None.to_string(), 2)]),
            transform_usage: BTreeMap::from([(TransformKind::Identity.to_string(), 2)]),
        },
        file_summaries: vec![FileSidecar {
            archive_path: "project/a.txt".to_string(),
            file_type: FileKind::Logs,
            size: first.len() as u64,
            hash: blake3::hash(&first).to_hex().to_string(),
            chunk_count: 1,
            entrypoint_hint: false,
            risk_hints: vec![],
        }],
        entrypoint_candidates: vec!["no obvious entrypoint detected".to_string()],
        dependency_hints: vec!["no dependency manifest detected".to_string()],
        risk_hints: vec![],
        extraction_map: vec![ExtractionHint {
            archive_path: "project/a.txt".to_string(),
            chunk_refs: vec![0],
        }],
        query_hints: vec!["type:logs".to_string()],
        aegisqr_integration_hints: vec![
            "use qr-max for smallest transport bundles".to_string(),
            "wrap manifest and sidecar hashes before encryption".to_string(),
        ],
    };

    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected orphan chunk rejection");
    assert!(error.to_string().contains("unreferenced chunk"));
}

#[test]
fn rejects_duplicate_chunk_references_within_file() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("duplicate-chunk-refs.aicx");
    let data = b"payload".to_vec();

    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Logs,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
        original_size: data.len() as u64,
        restored_size: data.len() as u64,
        file_hash: blake3::hash(&data).to_hex().to_string(),
        file_type: FileKind::Logs,
        detected_language: None,
        transform_applied: TransformKind::Identity,
        chunk_refs: vec![0, 0],
        executable_hint: false,
        risk_hints: vec![],
        metadata_flags: vec![],
    };
    let mut manifest = ArchiveManifest {
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: String::new(),
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

    #[derive(serde::Serialize)]
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

    let seed = ArchiveIdentitySeed {
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
    };
    manifest.archive_id = blake3::hash(
        serde_json::to_string(&seed)
            .expect("serialize seed")
            .as_bytes(),
    )
    .to_hex()
    .to_string();

    let sidecar = ArchiveSidecar {
        summary: SidecarSummary {
            total_files: manifest.file_count,
            total_size: manifest.original_total_size,
            compressed_size: manifest.stored_total_size,
            compression_ratio: manifest.compression_ratio,
            file_type_counts: BTreeMap::from([(FileKind::Logs.to_string(), 1)]),
            codec_usage: BTreeMap::from([(CodecKind::None.to_string(), 1)]),
            transform_usage: BTreeMap::from([(TransformKind::Identity.to_string(), 1)]),
        },
        file_summaries: vec![FileSidecar {
            archive_path: "project/readme.txt".to_string(),
            file_type: FileKind::Logs,
            size: data.len() as u64,
            hash: blake3::hash(&data).to_hex().to_string(),
            chunk_count: 2,
            entrypoint_hint: false,
            risk_hints: vec![],
        }],
        entrypoint_candidates: vec!["no obvious entrypoint detected".to_string()],
        dependency_hints: vec!["no dependency manifest detected".to_string()],
        risk_hints: vec![],
        extraction_map: vec![ExtractionHint {
            archive_path: "project/readme.txt".to_string(),
            chunk_refs: vec![0, 0],
        }],
        query_hints: vec!["type:logs".to_string()],
        aegisqr_integration_hints: vec![
            "use qr-max for smallest transport bundles".to_string(),
            "wrap manifest and sidecar hashes before encryption".to_string(),
        ],
    };

    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error =
        inspect_archive(&archive).expect_err("expected duplicate chunk reference rejection");
    assert!(error.to_string().contains("referenced more than once"));
}

#[cfg(unix)]
#[test]
fn unpack_ignores_intermediate_symlink_traversal_in_target_path() {
    use std::os::unix::fs::symlink;

    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("sample.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let safe_dir = temp.path().join("safe");
    let escape_dir = temp.path().join("escape");
    fs::create_dir_all(&safe_dir).expect("create safe");
    fs::create_dir_all(&escape_dir).expect("create escape");
    symlink(&escape_dir, safe_dir.join("sub")).expect("create symlink");

    let target = safe_dir.join("sub").join("..").join("restore");
    let manifest =
        unpack_archive(&archive, &target, Selection::default(), false).expect("unpack archive");

    assert_eq!(manifest.file_count, 1);
    assert!(safe_dir
        .join("restore")
        .join("project")
        .join("notes.txt")
        .exists());
    assert!(!escape_dir.join("restore").exists());
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
        magic: "AICX2".to_string(),
        version: 2,
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
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let restore = temp.path().join("restore");
    let error = unpack_archive(&archive, &restore, Selection::default(), false)
        .expect_err("expected traversal rejection");
    assert!(error.to_string().contains("unsafe archive path"));
}

#[test]
fn rejects_chunk_size_mismatches() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("chunk-size-mismatch.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64 + 1,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
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
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected stored-size validation failure");
    assert!(error
        .to_string()
        .contains("stored size does not match chunk length"));
}

#[test]
fn rejects_original_total_size_mismatches() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("original-size-mismatch.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: "test".to_string(),
        created_by: "test".to_string(),
        deterministic_build: true,
        profile: ArchiveProfile::Balanced,
        chunk_size: 4,
        original_total_size: data.len() as u64 + 1,
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
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected original-size validation failure");
    assert!(error
        .to_string()
        .contains("original size does not match file table"));
}

#[test]
fn rejects_non_contiguous_chunk_offsets() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("chunk-gap.aicx");
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
            offset: first.len() as u64 + 1,
            length: second.len() as u64,
            classification: FileKind::Text,
            compression_ratio: 1.0,
            transform_metadata: BTreeMap::new(),
        },
    ];
    let files = vec![
        FileRecord {
            archive_path: "project/readme.txt".to_string(),
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
            archive_path: "project/notes.txt".to_string(),
            original_path: Some("notes.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
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
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &[first, second].concat());

    let error =
        inspect_archive(&archive).expect_err("expected contiguous offset validation failure");
    assert!(error.to_string().contains("offset is not contiguous"));
}

#[test]
fn rejects_unsupported_transforms() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("unsupported-transform.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::JsonCanonical,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
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
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected transform validation failure");
    assert!(error.to_string().contains("unsupported transform"));
}

#[test]
fn rejects_tampered_sidecar() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("tampered-sidecar.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
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
    let mut sidecar = deterministic_sidecar(&manifest);
    sidecar.summary.total_files = 2;
    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected sidecar validation failure");
    assert!(error.to_string().contains("sidecar total file count"));
}

#[test]
fn rejects_tampered_sidecar_advisory_fields() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("tampered-sidecar-advisory.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
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
    let mut sidecar = deterministic_sidecar(&manifest);
    sidecar.entrypoint_candidates.push("evil".to_string());
    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error =
        inspect_archive(&archive).expect_err("expected advisory sidecar validation failure");
    assert!(error
        .to_string()
        .contains("sidecar contents do not match deterministic rebuild"));
}

#[test]
fn rejects_partial_sidecar() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("partial-sidecar.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
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
    let mut sidecar = deterministic_sidecar(&manifest);
    sidecar.file_summaries.pop();
    let envelope = ArchiveEnvelope {
        manifest: manifest.clone(),
        sidecar,
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected partial sidecar rejection");
    assert!(error
        .to_string()
        .contains("sidecar file summaries do not match manifest"));
}

#[test]
fn pack_emits_deterministic_manifests() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("deterministic.aicx");
    let manifest = pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    assert!(manifest.deterministic_build);
    assert_eq!(manifest.created_by, "aicx-rust");
    assert!(!manifest.archive_id.is_empty());
}

#[test]
fn manifest_and_sidecar_digests_are_deterministic() {
    let temp = tempdir().expect("tempdir");
    let input_dir = temp.path().join("project");
    fs::create_dir_all(&input_dir).expect("create input");
    fs::write(input_dir.join("notes.txt"), b"hello world\n").expect("write text");

    let archive = temp.path().join("digests.aicx");
    pack_archive(
        &[input_dir],
        &archive,
        PackOptions {
            chunk_size: 4,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        },
    )
    .expect("pack archive");

    let inspection = inspect_archive(&archive).expect("inspect archive");
    let manifest_hash = manifest_digest(&inspection.manifest).expect("manifest digest");
    let sidecar_hash = sidecar_digest(&inspection.sidecar).expect("sidecar digest");
    let digests = archive_digests(&archive).expect("archive digests");
    let report = report_archive(&archive).expect("report archive");

    assert_eq!(manifest_hash, inspection.manifest.archive_id);
    assert_eq!(
        sidecar_hash,
        sidecar_digest(&inspection.sidecar).expect("repeat digest")
    );
    assert_eq!(digests.manifest_digest, manifest_hash);
    assert_eq!(digests.sidecar_digest, sidecar_hash);
    assert_eq!(inspection.manifest_digest, manifest_hash);
    assert_eq!(inspection.sidecar_digest, sidecar_hash);
    assert_eq!(report.manifest_digest, manifest_hash);
    assert_eq!(report.sidecar_digest, sidecar_hash);
    assert_eq!(report.manifest.archive_id, manifest_hash);

    let mut tampered_sidecar = inspection.sidecar.clone();
    tampered_sidecar
        .entrypoint_candidates
        .push("evil".to_string());
    assert_ne!(
        sidecar_hash,
        sidecar_digest(&tampered_sidecar).expect("tampered digest")
    );
}

#[test]
fn rejects_archive_id_mismatch() {
    let temp = tempdir().expect("tempdir");
    let archive = temp.path().join("archive-id-mismatch.aicx");
    let data = b"payload".to_vec();
    let chunk = ChunkRecord {
        chunk_id: 0,
        file_id: 0,
        chunk_index: 0,
        original_size: data.len() as u64,
        stored_size: data.len() as u64,
        codec: CodecKind::None,
        codec_level: 0,
        transform: TransformKind::Identity,
        hash: blake3::hash(&data).to_hex().to_string(),
        offset: 0,
        length: data.len() as u64,
        classification: FileKind::Text,
        compression_ratio: 1.0,
        transform_metadata: BTreeMap::new(),
    };
    let file = FileRecord {
        archive_path: "project/readme.txt".to_string(),
        original_path: Some("readme.txt".to_string()),
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
        magic: "AICX2".to_string(),
        version: 2,
        archive_id: "not-the-right-id".to_string(),
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
        sidecar: deterministic_sidecar(&manifest),
    };
    write_archive(&archive, &envelope, &data);

    let error = inspect_archive(&archive).expect_err("expected archive-id validation failure");
    assert!(error
        .to_string()
        .contains("archive id does not match manifest contents"));
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
        magic: "AICX2".to_string(),
        version: 2,
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
        sidecar: deterministic_sidecar(&manifest),
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
