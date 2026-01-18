use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use indicatif::ProgressDrawTarget;
use extract_model_info_json::{ExtractStats, LineProgressReporter, ProgressReporter};
use extract_model_info_json::IndicatifProgressReporter;

#[test]
fn line_progress_reporter_writes_updates() {
    let writer = Cursor::new(Vec::new());
    let reporter = LineProgressReporter::with_writer(writer);

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
    let reporter = LineProgressReporter::with_writer(writer);

    reporter.on_start(Path::new("/tmp"));
    reporter.on_invalid_zip(Path::new("/tmp/bad.zip"), "invalid");

    let output = String::from_utf8(reporter.into_inner().into_inner()).unwrap();
    assert!(output.contains("invalid zip: /tmp/bad.zip"));
}

#[test]
fn line_progress_reporter_reports_invalid_zip_on_new_line_after_update() {
    let writer = Cursor::new(Vec::new());
    let reporter = LineProgressReporter::with_writer(writer);

    reporter.on_start(Path::new("/tmp"));
    reporter.on_update(&ExtractStats {
        directories_scanned: 1,
        safetensors_directories: 1,
        zip_files_checked: 1,
        extracted: 0,
    });
    reporter.on_invalid_zip(Path::new("/tmp/bad.zip"), "invalid");

    let output = String::from_utf8(reporter.into_inner().into_inner()).unwrap();
    assert!(output.contains("\ninvalid zip: /tmp/bad.zip"));
}

#[test]
fn line_progress_reporter_handles_concurrent_updates() {
    let reporter = Arc::new(LineProgressReporter::with_writer(Cursor::new(Vec::new())));
    reporter.on_start(Path::new("/tmp"));

    let mut handles = Vec::new();
    for index in 0..8 {
        let reporter = Arc::clone(&reporter);
        handles.push(thread::spawn(move || {
            reporter.on_update(&ExtractStats {
                directories_scanned: index + 1,
                safetensors_directories: 0,
                zip_files_checked: 0,
                extracted: 0,
            });
            reporter.on_invalid_zip(Path::new("/tmp/bad.zip"), "invalid");
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    reporter.on_finish(&ExtractStats {
        directories_scanned: 8,
        safetensors_directories: 0,
        zip_files_checked: 0,
        extracted: 0,
    });

    let reporter = match Arc::try_unwrap(reporter) {
        Ok(reporter) => reporter,
        Err(_) => panic!("reporter still shared"),
    };
    let output = String::from_utf8(reporter.into_inner().into_inner()).unwrap();
    assert!(output.contains("invalid zip: /tmp/bad.zip"));
}

#[test]
fn indicatif_progress_reporter_runs_with_hidden_target() {
    let reporter = IndicatifProgressReporter::with_draw_target(ProgressDrawTarget::hidden());

    reporter.on_start(Path::new("/tmp"));

    let stats = ExtractStats {
        directories_scanned: 1,
        safetensors_directories: 1,
        zip_files_checked: 1,
        extracted: 0,
    };

    reporter.on_update(&stats);
    reporter.on_invalid_zip(Path::new("/tmp/bad.zip"), "invalid");
    reporter.on_finish(&stats);
}
