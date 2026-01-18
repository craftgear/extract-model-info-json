use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use console::style;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
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
    fn on_start(&self, _root: &Path) {}

    fn on_update(&self, _stats: &ExtractStats) {}

    fn on_invalid_zip(&self, _zip_path: &Path, _reason: &str) {}

    fn on_finish(&self, _stats: &ExtractStats) {}
}

pub struct IndicatifProgressReporter {
    bar: ProgressBar,
}

impl IndicatifProgressReporter {
    pub fn new() -> Self {
        Self::with_draw_target(ProgressDrawTarget::stderr())
    }

    pub fn with_draw_target(draw_target: ProgressDrawTarget) -> Self {
        let bar = ProgressBar::with_draw_target(None, draw_target);
        let style = ProgressStyle::with_template("{spinner:.yellow} {msg:.blue}")
            .expect("invalid progress style template")
            .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷");
        bar.set_style(style);
        bar.enable_steady_tick(Duration::from_millis(120));

        Self { bar }
    }
}

impl ProgressReporter for IndicatifProgressReporter {
    fn on_start(&self, root: &Path) {
        let _ = self
            .bar
            .println(format!("scanning: {}", root.display()));
        self.bar.set_message(format_stats(&ExtractStats::default()));
    }

    fn on_update(&self, stats: &ExtractStats) {
        self.bar.set_message(format_stats(stats));
    }

    fn on_invalid_zip(&self, zip_path: &Path, reason: &str) {
        let message = format!("invalid zip: {} ({})", zip_path.display(), reason);
        let _ = self.bar.println(style(message).red().to_string());
    }

    fn on_finish(&self, stats: &ExtractStats) {
        self.bar.disable_steady_tick();
        self.bar.finish_with_message(format_stats(stats));
    }
}

struct LineProgressState<W: Write> {
    writer: W,
    last_stats: ExtractStats,
    started: bool,
}

pub struct LineProgressReporter<W: Write + Send> {
    state: Mutex<LineProgressState<W>>,
}

impl LineProgressReporter<std::io::Stderr> {
    pub fn new() -> Self {
        Self::with_writer(std::io::stderr())
    }
}

impl<W: Write + Send> LineProgressReporter<W> {
    pub fn with_writer(writer: W) -> Self {
        Self {
            state: Mutex::new(LineProgressState {
                writer,
                last_stats: ExtractStats::default(),
                started: false,
            }),
        }
    }

    pub fn into_inner(self) -> W {
        let state = match self.state.into_inner() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };
        state.writer
    }
}

impl<W: Write + Send> ProgressReporter for LineProgressReporter<W> {
    fn on_start(&self, root: &Path) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };

        if state.started {
            return;
        }

        let _ = writeln!(state.writer, "scanning: {}", root.display());
        let _ = state.writer.flush();
        state.started = true;
    }

    fn on_update(&self, stats: &ExtractStats) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };

        if *stats == state.last_stats {
            return;
        }

        let _ = write!(
            state.writer,
            "\rdirs: {} safetensors: {} zip: {} extracted: {}",
            stats.directories_scanned,
            stats.safetensors_directories,
            stats.zip_files_checked,
            stats.extracted
        );
        let _ = state.writer.flush();
        state.last_stats = *stats;
    }

    fn on_invalid_zip(&self, zip_path: &Path, reason: &str) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };

        let _ = write!(
            state.writer,
            "\ninvalid zip: {} ({})\n",
            zip_path.display(),
            reason
        );
        let _ = state.writer.flush();
    }

    fn on_finish(&self, stats: &ExtractStats) {
        self.on_update(stats);
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(err) => err.into_inner(),
        };
        let _ = writeln!(state.writer);
        let _ = state.writer.flush();
    }
}

fn format_stats(stats: &ExtractStats) -> String {
    format!(
        "dirs: {} zip: {} extracted: {}",
        stats.directories_scanned,
        stats.zip_files_checked,
        stats.extracted
    )
}

#[cfg(test)]
mod tests {
    use super::format_stats;
    use crate::domain::ExtractStats;

    #[test]
    fn format_stats_shows_dirs_zip_extracted_only() {
        let stats = ExtractStats {
            directories_scanned: 1,
            safetensors_directories: 99,
            zip_files_checked: 2,
            extracted: 3,
        };

        assert_eq!(format_stats(&stats), "dirs: 1 zip: 2 extracted: 3");
    }
}
