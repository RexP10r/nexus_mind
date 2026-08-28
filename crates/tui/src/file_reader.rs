use std::fs;
use std::path::PathBuf;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub content: String,
    pub size: u64,
}

pub struct FileReader<'a> {
    max_file_size: u64,
    supported_extensions: &'a [&'a str],
}

const SUPPORTED_EXTENSIONS: &[&str] = &[".md"];
impl <'a> FileReader <'_> {
    pub fn new(config: &Config) -> Self {
        Self {
            max_file_size: config.docs_max_file_size,
            supported_extensions: SUPPORTED_EXTENSIONS,
        }
    }

    pub fn read_files_flat(&self, path: &PathBuf) -> Vec<FileInfo> {
        let mut result = Vec::new();

        if path.is_file() {
            if let Some(file_info) = self.try_read_file(path) {
                result.push(file_info);
            }
            return result;
        }

        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_file() {
                        if let Some(file_info) = self.try_read_file(&entry_path) {
                            result.push(file_info);
                        }
                    }
                }
            }
        }

        result
    }

    pub fn read_files_recursive(&self, path: &PathBuf) -> Vec<FileInfo> {
        let mut result = Vec::new();

        if path.is_file() {
            if let Some(file_info) = self.try_read_file(path) {
                result.push(file_info);
            }
            return result;
        }

        if path.is_dir() {
            self.walk_dir(path, &mut result);
        }

        result
    }

    fn walk_dir(&self, dir: &PathBuf, result: &mut Vec<FileInfo>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.walk_dir(&path, result);
                } else if path.is_file() {
                    if let Some(file_info) = self.try_read_file(&path) {
                        result.push(file_info);
                    }
                }
            }
        }
    }

    fn try_read_file(&self, path: &PathBuf) -> Option<FileInfo> {
        if !path.is_file() {
            return None;
        }

        if !self.has_supported_extension(path) {
            return None;
        }

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return None,
        };

        let size = metadata.len();
        if size > self.max_file_size {
            return None;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return None,
        };

        if is_binary(content.as_bytes()) {
            return None;
        }

        Some(FileInfo {
            path: path.clone(),
            content,
            size,
        })
    }

    fn has_supported_extension(&self, path: &PathBuf) -> bool {
        if let Some(ext) = path.extension() {
            let ext_str = format!(".{}", ext.to_string_lossy());
            self.supported_extensions.iter().any(|e| e == &ext_str)
        } else {
            false
        }
    }
}

fn is_binary(content: &[u8]) -> bool {
    let check_len = content.len().min(8192);
    content[..check_len].contains(&0)
}
