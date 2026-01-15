pub const MODEL_INFO_FILE_NAME: &str = "model_info.json";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExtractStats {
    pub directories_scanned: u64,
    pub safetensors_directories: u64,
    pub zip_files_checked: u64,
    pub extracted: u64,
}
