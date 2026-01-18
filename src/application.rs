use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;

use crate::domain::{ExtractStats, MODEL_INFO_FILE_NAME};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub trait FilePorts: Send + Sync {
    fn for_each_directory(
        &self,
        root: &Path,
        on_dir: &mut dyn FnMut(PathBuf) -> Result<(), ExtractError>,
    ) -> Result<(), ExtractError>;
    fn list_files_in_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, ExtractError>;
    fn extract_zip_entry_if_exists(
        &self,
        zip_path: &Path,
        entry_name: &str,
        output_dir: &Path,
    ) -> Result<ZipEntryOutcome, ExtractError>;
}

pub trait ProgressReporter: Send + Sync {
    fn on_start(&self, root: &Path);
    fn on_update(&self, stats: &ExtractStats);
    fn on_invalid_zip(&self, zip_path: &Path, reason: &str);
    fn on_finish(&self, stats: &ExtractStats);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZipEntryOutcome {
    Extracted,
    NotFound,
    InvalidZip(String),
}

struct AtomicExtractStats {
    directories_scanned: AtomicU64,
    safetensors_directories: AtomicU64,
    zip_files_checked: AtomicU64,
    extracted: AtomicU64,
}

impl AtomicExtractStats {
    fn new() -> Self {
        Self {
            directories_scanned: AtomicU64::new(0),
            safetensors_directories: AtomicU64::new(0),
            zip_files_checked: AtomicU64::new(0),
            extracted: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> ExtractStats {
        ExtractStats {
            directories_scanned: self.directories_scanned.load(Ordering::Relaxed),
            safetensors_directories: self.safetensors_directories.load(Ordering::Relaxed),
            zip_files_checked: self.zip_files_checked.load(Ordering::Relaxed),
            extracted: self.extracted.load(Ordering::Relaxed),
        }
    }

    fn increment_directories(&self) {
        self.directories_scanned.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_safetensors_directories(&self) {
        self.safetensors_directories.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_zip_files_checked(&self) {
        self.zip_files_checked.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_extracted(&self) {
        self.extracted.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn extract_model_info(
    ports: &dyn FilePorts,
    progress: &dyn ProgressReporter,
    root: &Path,
) -> Result<ExtractStats, ExtractError> {
    let stats = AtomicExtractStats::new();

    progress.on_start(root);

    let mut directories = Vec::new();
    ports.for_each_directory(root, &mut |dir_path| {
        directories.push(dir_path);
        Ok::<(), ExtractError>(())
    })?;

    directories.par_iter().try_for_each(|dir_path| {
        stats.increment_directories();

        let files = ports.list_files_in_dir(dir_path)?;
        let mut has_safetensors = false;
        let mut zip_files = Vec::new();

        for file in files {
            match file.extension() {
                Some(ext) if ext == OsStr::new("safetensors") => {
                    has_safetensors = true;
                }
                Some(ext) if ext == OsStr::new("zip") => {
                    zip_files.push(file);
                }
                _ => {}
            }
        }

        if has_safetensors {
            stats.increment_safetensors_directories();
            let snapshot = stats.snapshot();
            progress.on_update(&snapshot);

            for zip_path in zip_files {
                stats.increment_zip_files_checked();

                let outcome = ports.extract_zip_entry_if_exists(
                    &zip_path,
                    MODEL_INFO_FILE_NAME,
                    dir_path,
                )?;

                match outcome {
                    ZipEntryOutcome::Extracted => {
                        stats.increment_extracted();
                    }
                    ZipEntryOutcome::InvalidZip(reason) => {
                        progress.on_invalid_zip(&zip_path, &reason);
                    }
                    ZipEntryOutcome::NotFound => {}
                }

                let snapshot = stats.snapshot();
                progress.on_update(&snapshot);
            }
        } else {
            let snapshot = stats.snapshot();
            progress.on_update(&snapshot);
        }

        Ok::<(), ExtractError>(())
    })?;

    let final_stats = stats.snapshot();
    progress.on_finish(&final_stats);

    Ok(final_stats)
}
