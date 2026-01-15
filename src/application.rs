use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::domain::{ExtractStats, MODEL_INFO_FILE_NAME};

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub trait FilePorts {
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
    ) -> Result<bool, ExtractError>;
}

pub trait ProgressReporter {
    fn on_start(&mut self, root: &Path);
    fn on_update(&mut self, stats: &ExtractStats);
    fn on_finish(&mut self, stats: &ExtractStats);
}

pub fn extract_model_info(
    ports: &dyn FilePorts,
    progress: &mut dyn ProgressReporter,
    root: &Path,
) -> Result<ExtractStats, ExtractError> {
    let mut stats = ExtractStats::default();

    progress.on_start(root);

    ports.for_each_directory(root, &mut |dir_path| {
        stats.directories_scanned += 1;

        let files = ports.list_files_in_dir(&dir_path)?;
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
            stats.safetensors_directories += 1;
            progress.on_update(&stats);

            for zip_path in zip_files {
                stats.zip_files_checked += 1;

                let extracted = ports.extract_zip_entry_if_exists(
                    &zip_path,
                    MODEL_INFO_FILE_NAME,
                    &dir_path,
                )?;
                if extracted {
                    stats.extracted += 1;
                }

                progress.on_update(&stats);
            }
        } else {
            progress.on_update(&stats);
        }

        Ok(())
    })?;

    progress.on_finish(&stats);

    Ok(stats)
}
