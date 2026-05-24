use std::path::Path;

use crate::model::FileKind;

fn extension_lower(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn basename_lower(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn is_probably_text(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let printable = data
        .iter()
        .filter(|byte| matches!(byte, b'\n' | b'\r' | b'\t' | 0x20..=0x7e))
        .count();
    printable * 100 / data.len() >= 85
}

fn contains_magic(data: &[u8], magic: &[u8]) -> bool {
    data.len() >= magic.len() && &data[..magic.len()] == magic
}

pub fn classify(path: &str, data: &[u8]) -> FileKind {
    let ext = extension_lower(path);
    let basename = basename_lower(path);

    if contains_magic(data, b"\x1f\x8b") || contains_magic(data, b"\xfd7zXZ") {
        return FileKind::Archive;
    }
    if contains_magic(data, b"PK\x03\x04")
        || contains_magic(data, b"\x89PNG")
        || contains_magic(data, b"\xff\xd8\xff")
        || contains_magic(data, b"%PDF")
    {
        return FileKind::Media;
    }
    if ext.as_deref() == Some("json") {
        return FileKind::Json;
    }
    if matches!(ext.as_deref(), Some("yml" | "yaml")) {
        return FileKind::Yaml;
    }
    if ext.as_deref() == Some("xml") {
        return FileKind::Xml;
    }
    if ext.as_deref() == Some("toml") {
        return FileKind::Toml;
    }
    if ext.as_deref() == Some("md") {
        return FileKind::Markdown;
    }
    if ext.as_deref() == Some("csv") {
        return FileKind::Csv;
    }
    if matches!(ext.as_deref(), Some("log" | "txt")) && is_probably_text(data) {
        return FileKind::Logs;
    }
    if let Some(name) = basename.as_deref() {
        if matches!(
            name,
            "dockerfile" | "makefile" | "cargo.toml" | "package.json" | "pyproject.toml"
        ) {
            return FileKind::SourceCode;
        }
    }
    if data.contains(&0) {
        return FileKind::Binary;
    }
    if std::str::from_utf8(data).is_ok() {
        if data.starts_with(b"#!") {
            return FileKind::SourceCode;
        }
        if let Ok(text) = std::str::from_utf8(data) {
            let trimmed = text.trim_start();
            if (trimmed.starts_with('{') || trimmed.starts_with('['))
                && serde_json::from_str::<serde_json::Value>(text).is_ok()
            {
                return FileKind::Json;
            }
            if trimmed.starts_with('<') && trimmed.contains('>') {
                return FileKind::Xml;
            }
            if text
                .lines()
                .any(|line| line.contains(':') && line.contains(' '))
            {
                return FileKind::Yaml;
            }
            if text.lines().any(|line| line.contains(',')) {
                return FileKind::Csv;
            }
            if is_probably_text(data) {
                return FileKind::Text;
            }
        }
    }
    FileKind::Unknown
}

pub fn detect_language(path: &str) -> Option<String> {
    let ext = extension_lower(path)?;
    let language = match ext.as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "sh" => "shell",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "md" => "markdown",
        _ => return None,
    };
    Some(language.to_string())
}
