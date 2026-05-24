use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{AicxError, Result};

pub fn validate_archive_path(raw: &str) -> Result<PathBuf> {
    let path = Path::new(raw);
    if raw.is_empty() {
        return Err(AicxError::UnsafePath("empty archive path".to_string()));
    }
    if path.is_absolute() {
        return Err(AicxError::UnsafePath(format!(
            "absolute archive path rejected: {raw}"
        )));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(AicxError::UnsafePath(format!(
                    "unsafe archive path rejected: {raw}"
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(AicxError::UnsafePath(format!(
            "archive path resolves to empty: {raw}"
        )));
    }
    Ok(normalized)
}

pub fn archive_path_to_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy()),
            _ => None,
        })
        .map(|part| part.into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn safe_output_path(root: &Path, archive_path: &str) -> Result<PathBuf> {
    let relative = validate_archive_path(archive_path)?;
    Ok(root.join(relative))
}

pub fn normalize_directory_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(AicxError::UnsafePath(
                        "unsafe parent chain component encountered".to_string(),
                    ));
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

pub fn is_filesystem_root(path: &Path) -> bool {
    let mut saw_component = false;
    let mut saw_root = false;
    for component in path.components() {
        saw_component = true;
        match component {
            Component::CurDir | Component::ParentDir | Component::Normal(_) => return false,
            Component::Prefix(_) => {}
            Component::RootDir => saw_root = true,
        }
    }
    saw_component && saw_root
}

pub fn ensure_parent_chain_safe(path: &Path, root: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AicxError::UnsafePath("missing parent directory".to_string()))?;
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| AicxError::UnsafePath("target path escapes restore root".to_string()))?;
    let mut current = root.to_path_buf();
    if !current.exists() {
        fs::create_dir_all(&current)?;
    }
    for component in relative.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => {
                current.push(part);
                if current.exists() {
                    let metadata = fs::symlink_metadata(&current)?;
                    if metadata.file_type().is_symlink() {
                        return Err(AicxError::UnsafePath(format!(
                            "symlink escape rejected: {}",
                            current.display()
                        )));
                    }
                    if !metadata.is_dir() {
                        return Err(AicxError::UnsafePath(format!(
                            "non-directory path blocks restore: {}",
                            current.display()
                        )));
                    }
                } else {
                    fs::create_dir(&current)?;
                }
            }
            _ => {
                return Err(AicxError::UnsafePath(
                    "unsafe parent chain component encountered".to_string(),
                ));
            }
        }
    }
    Ok(())
}

pub fn ensure_directory_chain_safe(path: &Path) -> Result<()> {
    let normalized = normalize_directory_path(path)?;
    if normalized.as_os_str().is_empty() {
        return Ok(());
    }

    let mut current = PathBuf::new();
    for component in normalized.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => {
                current.push(part);
                if current.exists() {
                    let metadata = fs::symlink_metadata(&current)?;
                    if metadata.file_type().is_symlink() {
                        return Err(AicxError::UnsafePath(format!(
                            "symlink escape rejected: {}",
                            current.display()
                        )));
                    }
                    if !metadata.is_dir() {
                        return Err(AicxError::UnsafePath(format!(
                            "non-directory path blocks creation: {}",
                            current.display()
                        )));
                    }
                } else {
                    fs::create_dir(&current)?;
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                current.push(component.as_os_str());
                if current.exists() {
                    let metadata = fs::symlink_metadata(&current)?;
                    if metadata.file_type().is_symlink() {
                        return Err(AicxError::UnsafePath(format!(
                            "symlink escape rejected: {}",
                            current.display()
                        )));
                    }
                    if !metadata.is_dir() {
                        return Err(AicxError::UnsafePath(format!(
                            "non-directory path blocks creation: {}",
                            current.display()
                        )));
                    }
                }
            }
            Component::ParentDir => {
                return Err(AicxError::UnsafePath(
                    "unsafe parent chain component encountered".to_string(),
                ));
            }
        }
    }
    Ok(())
}
