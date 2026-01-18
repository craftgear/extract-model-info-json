use std::fs;
use std::io::Write;
use std::path::Path;

use extract_model_info_json::{
    extract_model_info, FsPorts, NoProgressReporter, MODEL_INFO_FILE_NAME,
};

fn create_zip(path: &Path, entries: Vec<(&str, &str)>) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::default();

    for (name, contents) in entries {
        zip.start_file(name, options)?;
        zip.write_all(contents.as_bytes())?;
    }

    zip.finish()?;
    Ok(())
}

#[test]
fn extracts_model_info_json_from_zip_in_safetensors_dir() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let model_dir = temp_dir.path().join("model");
    fs::create_dir_all(&model_dir)?;

    fs::write(model_dir.join("model.safetensors"), b"")?;
    create_zip(
        &model_dir.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "{\"a\": 1}")],
    )?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    let extracted = fs::read_to_string(model_dir.join(MODEL_INFO_FILE_NAME))?;
    assert_eq!(extracted, "{\"a\": 1}");
    assert_eq!(stats.extracted, 1);

    Ok(())
}

#[test]
fn skips_zip_without_model_info_json() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let model_dir = temp_dir.path().join("model");
    fs::create_dir_all(&model_dir)?;

    fs::write(model_dir.join("model.safetensors"), b"")?;
    create_zip(&model_dir.join("model.zip"), vec![("other.json", "{}")])?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    assert!(!model_dir.join(MODEL_INFO_FILE_NAME).exists());
    assert_eq!(stats.extracted, 0);

    Ok(())
}

#[test]
fn ignores_zip_in_directory_without_safetensors() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;

    let no_safe_dir = temp_dir.path().join("no_safe");
    fs::create_dir_all(&no_safe_dir)?;
    create_zip(
        &no_safe_dir.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "{\"no\": 1}")],
    )?;

    let safe_dir = temp_dir.path().join("safe");
    fs::create_dir_all(&safe_dir)?;
    fs::write(safe_dir.join("model.safetensors"), b"")?;
    create_zip(
        &safe_dir.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "{\"yes\": 1}")],
    )?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    assert!(!no_safe_dir.join(MODEL_INFO_FILE_NAME).exists());
    assert!(safe_dir.join(MODEL_INFO_FILE_NAME).exists());
    assert_eq!(stats.extracted, 1);

    Ok(())
}

#[test]
fn overwrites_existing_model_info_json() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let model_dir = temp_dir.path().join("model");
    fs::create_dir_all(&model_dir)?;

    fs::write(model_dir.join("model.safetensors"), b"")?;
    fs::write(model_dir.join(MODEL_INFO_FILE_NAME), "old")?;
    create_zip(
        &model_dir.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "new")],
    )?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let _stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    let extracted = fs::read_to_string(model_dir.join(MODEL_INFO_FILE_NAME))?;
    assert_eq!(extracted, "new");

    Ok(())
}

#[test]
fn extracts_from_nested_directories() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let nested_dir = temp_dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested_dir)?;

    fs::write(nested_dir.join("model.safetensors"), b"")?;
    create_zip(
        &nested_dir.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "nested")],
    )?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    let extracted = fs::read_to_string(nested_dir.join(MODEL_INFO_FILE_NAME))?;
    assert_eq!(extracted, "nested");
    assert_eq!(stats.extracted, 1);

    Ok(())
}

#[test]
fn reports_stats_for_scanned_directories() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let dir_a = temp_dir.path().join("a");
    let dir_b = temp_dir.path().join("b");

    fs::create_dir_all(&dir_a)?;
    fs::create_dir_all(&dir_b)?;
    fs::write(dir_a.join("model.safetensors"), b"")?;
    create_zip(
        &dir_a.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "ok")],
    )?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    assert!(stats.directories_scanned >= 2);
    assert_eq!(stats.safetensors_directories, 1);
    assert_eq!(stats.zip_files_checked, 1);
    assert_eq!(stats.extracted, 1);

    Ok(())
}

#[test]
fn continues_when_zip_is_invalid() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let bad_dir = temp_dir.path().join("bad");
    let good_dir = temp_dir.path().join("good");

    fs::create_dir_all(&bad_dir)?;
    fs::create_dir_all(&good_dir)?;

    fs::write(bad_dir.join("model.safetensors"), b"")?;
    fs::write(bad_dir.join("broken.zip"), b"not a zip")?;

    fs::write(good_dir.join("model.safetensors"), b"")?;
    create_zip(
        &good_dir.join("model.zip"),
        vec![(MODEL_INFO_FILE_NAME, "ok")],
    )?;

    let ports = FsPorts::new();
    let progress = NoProgressReporter::new();
    let stats = extract_model_info(&ports, &progress, temp_dir.path())?;

    assert!(good_dir.join(MODEL_INFO_FILE_NAME).exists());
    assert_eq!(stats.extracted, 1);

    Ok(())
}
