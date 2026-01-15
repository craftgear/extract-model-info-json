use std::io::Cursor;
use std::path::Path;

use extract_model_info_json::{ExtractStats, LineProgressReporter, ProgressReporter};

#[test]
fn line_progress_reporter_writes_updates() {
    let writer = Cursor::new(Vec::new());
    let mut reporter = LineProgressReporter::with_writer(writer);

    reporter.on_start(Path::new("/tmp"));

    let stats = ExtractStats {
        directories_scanned: 2,
        safetensors_directories: 1,
        zip_files_checked: 1,
        extracted: 1,
    };

    reporter.on_update(&stats);
    reporter.on_finish(&stats);

    let output = String::from_utf8(reporter.into_inner().into_inner()).unwrap();
    assert!(output.contains("scanning: /tmp"));
    assert!(output.contains("dirs: 2"));
}

#[test]
fn line_progress_reporter_reports_invalid_zip() {
    let writer = Cursor::new(Vec::new());
    let mut reporter = LineProgressReporter::with_writer(writer);

    reporter.on_start(Path::new("/tmp"));
    reporter.on_invalid_zip(Path::new("/tmp/bad.zip"), "invalid");

    let output = String::from_utf8(reporter.into_inner().into_inner()).unwrap();
    assert!(output.contains("invalid zip: /tmp/bad.zip"));
}
