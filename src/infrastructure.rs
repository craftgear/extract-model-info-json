use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::application::{ExtractError, FilePorts, ProgressReporter, ZipEntryOutcome};
use crate::domain::ExtractStats;

pub struct FsPorts;

impl FsPorts {
    pub fn new() -> Self {
        Self
    }
}

impl FilePorts for FsPorts {
    fn for_each_directory(
        &self,
        root: &Path,
        on_dir: &mut dyn FnMut(PathBuf) -> Result<(), ExtractError>,
    ) -> Result<(), ExtractError> {
        for entry in WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(|err| ExtractError::Message(err.to_string()))?;

            if entry.file_type().is_dir() {
                on_dir(entry.path().to_path_buf())?;
            }
        }

        Ok(())
    }

    fn list_files_in_dir(&self, dir: &Path) -> Result<Vec<PathBuf>, ExtractError> {
        let mut files = Vec::new();

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if file_type.is_file() {
                files.push(entry.path());
            }
        }

        Ok(files)
    }

    fn extract_zip_entry_if_exists(
        &self,
        zip_path: &Path,
        entry_name: &str,
        output_dir: &Path,
    ) -> Result<ZipEntryOutcome, ExtractError> {
        let file = match fs::File::open(zip_path) {
            Ok(file) => file,
            Err(err) => {
                // 破損や読み取り不能でも全体処理を止めないため
                return Ok(ZipEntryOutcome::InvalidZip(err.to_string()));
            }
        };
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(err) => {
                return Ok(ZipEntryOutcome::InvalidZip(err.to_string()));
            }
        };

        for index in 0..archive.len() {
            let mut entry = match archive.by_index(index) {
                Ok(entry) => entry,
                Err(err) => {
                    return Ok(ZipEntryOutcome::InvalidZip(err.to_string()));
                }
            };

            if entry.is_dir() {
                continue;
            }

            let entry_path = Path::new(entry.name());
            let entry_file_name = entry_path.file_name();

            if entry_file_name == Some(OsStr::new(entry_name)) {
                let output_path = output_dir.join(entry_name);
                let mut output_file = match fs::File::create(output_path) {
                    Ok(output_file) => output_file,
                    Err(err) => {
                        return Ok(ZipEntryOutcome::InvalidZip(err.to_string()));
                    }
                };
                if let Err(err) = io::copy(&mut entry, &mut output_file) {
                    return Ok(ZipEntryOutcome::InvalidZip(err.to_string()));
                }

                return Ok(ZipEntryOutcome::Extracted);
            }
        }

        Ok(ZipEntryOutcome::NotFound)
    }
}

pub struct NoProgressReporter;

impl NoProgressReporter {
    pub fn new() -> Self {
        Self
    }
}

impl ProgressReporter for NoProgressReporter {
    fn on_start(&mut self, _root: &Path) {}

    fn on_update(&mut self, _stats: &ExtractStats) {}

    fn on_invalid_zip(&mut self, _zip_path: &Path, _reason: &str) {}

    fn on_finish(&mut self, _stats: &ExtractStats) {}
}

pub struct LineProgressReporter<W: Write> {
    writer: W,
    last_stats: ExtractStats,
    started: bool,
}

impl LineProgressReporter<std::io::Stderr> {
    pub fn new() -> Self {
        Self::with_writer(std::io::stderr())
    }
}

impl<W: Write> LineProgressReporter<W> {
    pub fn with_writer(writer: W) -> Self {
        Self {
            writer,
            last_stats: ExtractStats::default(),
            started: false,
        }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> ProgressReporter for LineProgressReporter<W> {
    fn on_start(&mut self, root: &Path) {
        if self.started {
            return;
        }

        let _ = writeln!(self.writer, "scanning: {}", root.display());
        let _ = self.writer.flush();
        self.started = true;
    }

    fn on_update(&mut self, stats: &ExtractStats) {
        if *stats == self.last_stats {
            return;
        }

        let _ = write!(
            self.writer,
            "\rdirs: {} safetensors: {} zip: {} extracted: {}",
            stats.directories_scanned,
            stats.safetensors_directories,
            stats.zip_files_checked,
            stats.extracted
        );
        let _ = self.writer.flush();
        self.last_stats = *stats;
    }

    fn on_invalid_zip(&mut self, zip_path: &Path, reason: &str) {
        let _ = write!(
            self.writer,
            "\rinvalid zip: {} ({})\n",
            zip_path.display(),
            reason
        );
        let _ = self.writer.flush();
    }

    fn on_finish(&mut self, stats: &ExtractStats) {
        self.on_update(stats);
        let _ = writeln!(self.writer);
        let _ = self.writer.flush();
    }
}
