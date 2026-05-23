use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArchiveProfile {
    Fast,
    Balanced,
    Max,
    QrMax,
    Agent,
    Secure,
}

impl Default for ArchiveProfile {
    fn default() -> Self {
        Self::Balanced
    }
}

impl Display for ArchiveProfile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Max => "max",
            Self::QrMax => "qr-max",
            Self::Agent => "agent",
            Self::Secure => "secure",
        })
    }
}

impl FromStr for ArchiveProfile {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "fast" => Ok(Self::Fast),
            "balanced" => Ok(Self::Balanced),
            "max" => Ok(Self::Max),
            "qr-max" => Ok(Self::QrMax),
            "agent" => Ok(Self::Agent),
            "secure" => Ok(Self::Secure),
            other => Err(format!("unknown profile: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    Blake3,
    Sha256,
}

impl Default for HashAlgorithm {
    fn default() -> Self {
        Self::Blake3
    }
}

impl Display for HashAlgorithm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Blake3 => "blake3",
            Self::Sha256 => "sha256",
        })
    }
}

impl FromStr for HashAlgorithm {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "blake3" => Ok(Self::Blake3),
            "sha256" => Ok(Self::Sha256),
            other => Err(format!("unknown hash algorithm: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecKind {
    None,
    Zstd,
    Lz4,
    Gzip,
    Xz,
}

impl Display for CodecKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::None => "none",
            Self::Zstd => "zstd",
            Self::Lz4 => "lz4",
            Self::Gzip => "gzip",
            Self::Xz => "xz",
        })
    }
}

impl FromStr for CodecKind {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "zstd" => Ok(Self::Zstd),
            "lz4" => Ok(Self::Lz4),
            "gzip" => Ok(Self::Gzip),
            "xz" => Ok(Self::Xz),
            other => Err(format!("unknown codec: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Json,
    Yaml,
    Xml,
    Toml,
    Markdown,
    Text,
    SourceCode,
    Logs,
    Csv,
    Binary,
    Archive,
    Media,
    Unknown,
}

impl Display for FileKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Xml => "xml",
            Self::Toml => "toml",
            Self::Markdown => "markdown",
            Self::Text => "text",
            Self::SourceCode => "source_code",
            Self::Logs => "logs",
            Self::Csv => "csv",
            Self::Binary => "binary",
            Self::Archive => "archive",
            Self::Media => "media",
            Self::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformKind {
    Identity,
    JsonCanonical,
}

impl Display for TransformKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Identity => "identity",
            Self::JsonCanonical => "json_canonical",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub archive_path: String,
    pub original_path: Option<String>,
    pub original_size: u64,
    pub restored_size: u64,
    pub file_hash: String,
    pub file_type: FileKind,
    pub detected_language: Option<String>,
    pub transform_applied: TransformKind,
    pub chunk_refs: Vec<u64>,
    pub executable_hint: bool,
    pub risk_hints: Vec<String>,
    #[serde(default)]
    pub metadata_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub chunk_id: u64,
    pub file_id: u64,
    pub chunk_index: u64,
    pub original_size: u64,
    pub stored_size: u64,
    pub codec: CodecKind,
    pub codec_level: i32,
    pub transform: TransformKind,
    pub hash: String,
    pub offset: u64,
    pub length: u64,
    pub classification: FileKind,
    pub compression_ratio: f64,
    #[serde(default)]
    pub transform_metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarSummary {
    pub total_files: u64,
    pub total_size: u64,
    pub compressed_size: u64,
    pub compression_ratio: f64,
    pub file_type_counts: BTreeMap<String, u64>,
    pub codec_usage: BTreeMap<String, u64>,
    pub transform_usage: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSidecar {
    pub archive_path: String,
    pub file_type: FileKind,
    pub size: u64,
    pub hash: String,
    pub chunk_count: u64,
    pub entrypoint_hint: bool,
    pub risk_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionHint {
    pub archive_path: String,
    pub chunk_refs: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSidecar {
    pub summary: SidecarSummary,
    pub file_summaries: Vec<FileSidecar>,
    pub entrypoint_candidates: Vec<String>,
    pub dependency_hints: Vec<String>,
    pub risk_hints: Vec<String>,
    pub extraction_map: Vec<ExtractionHint>,
    pub query_hints: Vec<String>,
    pub aegisqr_integration_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub magic: String,
    pub version: u32,
    pub archive_id: String,
    pub created_by: String,
    pub deterministic_build: bool,
    pub profile: ArchiveProfile,
    pub chunk_size: u64,
    pub original_total_size: u64,
    pub stored_total_size: u64,
    pub compression_ratio: f64,
    pub file_count: u64,
    pub chunk_count: u64,
    pub hash_algorithm: HashAlgorithm,
    pub data_hash: String,
    pub files: Vec<FileRecord>,
    pub chunks: Vec<ChunkRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEnvelope {
    pub manifest: ArchiveManifest,
    pub sidecar: ArchiveSidecar,
}

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub chunk_size: usize,
    pub profile: ArchiveProfile,
    pub hash_algorithm: HashAlgorithm,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            chunk_size: 4 * 1024 * 1024,
            profile: ArchiveProfile::Balanced,
            hash_algorithm: HashAlgorithm::Blake3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub paths: Vec<String>,
    pub exact: bool,
}

impl Selection {
    pub fn matches(&self, archive_path: &str) -> bool {
        if self.paths.is_empty() {
            return true;
        }
        self.paths.iter().any(|candidate| {
            if self.exact {
                archive_path == candidate
            } else {
                archive_path == candidate
                    || archive_path
                        .strip_prefix(candidate)
                        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveVerification {
    pub valid: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveReport {
    pub manifest: ArchiveManifest,
    pub sidecar: ArchiveSidecar,
    pub verification: ArchiveVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInspection {
    pub manifest: ArchiveManifest,
    pub sidecar: ArchiveSidecar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileComparisonRow {
    pub profile: ArchiveProfile,
    pub stored_size: u64,
    pub original_size: u64,
    pub ratio: f64,
}
