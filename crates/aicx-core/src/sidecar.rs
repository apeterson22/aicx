use std::collections::BTreeMap;

use crate::model::{
    ArchiveManifest, ArchiveSidecar, ExtractionHint, FileKind, FileSidecar, SidecarSummary,
};

fn entrypoint_candidate(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    matches!(
        name.as_str(),
        "main.py"
            | "app.py"
            | "index.js"
            | "package.json"
            | "cargo.toml"
            | "pyproject.toml"
            | "dockerfile"
            | "docker-compose.yml"
            | "makefile"
    )
}

fn dependency_hint(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    matches!(
        name.as_str(),
        "cargo.toml"
            | "package.json"
            | "pyproject.toml"
            | "requirements.txt"
            | "poetry.lock"
            | "go.mod"
            | "pom.xml"
            | "build.gradle"
            | "makefile"
    )
}

fn risk_hints(path: &str, kind: FileKind, size: u64) -> Vec<String> {
    let mut hints = Vec::new();
    let lower = path.to_ascii_lowercase();
    if lower.contains(".github/workflows/") && (lower.ends_with(".yml") || lower.ends_with(".yaml"))
    {
        hints.push("contains GitHub Actions workflow".to_string());
    }
    if lower.ends_with("dockerfile") {
        hints.push("contains Dockerfile".to_string());
    }
    if lower.contains("kubernetes") || lower.contains("k8s") || lower.ends_with(".k8s.yaml") {
        hints.push("contains Kubernetes YAML".to_string());
    }
    if lower.contains(".env") {
        hints.push("contains .env file".to_string());
    }
    if lower.contains("private") || lower.contains("secret") || lower.contains("key") {
        hints.push("contains private key pattern".to_string());
    }
    if lower.ends_with(".sh") || lower.ends_with(".exe") || lower.ends_with(".bin") {
        hints.push("script file".to_string());
    }
    if matches!(kind, FileKind::Binary | FileKind::Archive | FileKind::Media) {
        hints.push("binary payload".to_string());
    }
    if matches!(kind, FileKind::Archive) {
        hints.push("contains compressed payload".to_string());
    }
    if size > 10 * 1024 * 1024 {
        hints.push("contains large file".to_string());
    }
    hints
}

pub fn build_sidecar(manifest: &ArchiveManifest) -> ArchiveSidecar {
    let mut file_type_counts = BTreeMap::new();
    let mut codec_usage = BTreeMap::new();
    let mut transform_usage = BTreeMap::new();
    let mut file_summaries = Vec::new();
    let mut entrypoints = Vec::new();
    let mut dependencies = Vec::new();
    let mut risks = Vec::new();
    let mut extraction_map = Vec::new();
    let mut query_hints = Vec::new();

    for file in &manifest.files {
        *file_type_counts
            .entry(file.file_type.to_string())
            .or_insert(0) += 1;
        if entrypoint_candidate(&file.archive_path) {
            entrypoints.push(file.archive_path.clone());
        }
        if dependency_hint(&file.archive_path) {
            dependencies.push(file.archive_path.clone());
        }
        let file_risks = risk_hints(&file.archive_path, file.file_type, file.original_size);
        risks.extend(file_risks.clone());
        file_summaries.push(FileSidecar {
            archive_path: file.archive_path.clone(),
            file_type: file.file_type,
            size: file.original_size,
            hash: file.file_hash.clone(),
            chunk_count: file.chunk_refs.len() as u64,
            entrypoint_hint: entrypoint_candidate(&file.archive_path),
            risk_hints: file_risks,
        });
        extraction_map.push(ExtractionHint {
            archive_path: file.archive_path.clone(),
            chunk_refs: file.chunk_refs.clone(),
        });
        query_hints.push(format!("type:{}", file.file_type));
    }

    for chunk in &manifest.chunks {
        *codec_usage.entry(chunk.codec.to_string()).or_insert(0) += 1;
        *transform_usage
            .entry(chunk.transform.to_string())
            .or_insert(0) += 1;
    }

    let summary = SidecarSummary {
        total_files: manifest.file_count,
        total_size: manifest.original_total_size,
        compressed_size: manifest.stored_total_size,
        compression_ratio: manifest.compression_ratio,
        file_type_counts,
        codec_usage,
        transform_usage,
    };

    if entrypoints.is_empty() {
        entrypoints.push("no obvious entrypoint detected".to_string());
    }
    if dependencies.is_empty() {
        dependencies.push("no dependency manifest detected".to_string());
    }

    let mut aegisqr_hints = vec![
        "use qr-max for smallest transport bundles".to_string(),
        "wrap manifest and sidecar hashes before encryption".to_string(),
    ];
    if manifest.compression_ratio > 1.0 {
        aegisqr_hints.push("archive is already efficiently compressed".to_string());
    }

    ArchiveSidecar {
        summary,
        file_summaries,
        entrypoint_candidates: entrypoints,
        dependency_hints: dependencies,
        risk_hints: risks,
        extraction_map,
        query_hints,
        aegisqr_integration_hints: aegisqr_hints,
    }
}
