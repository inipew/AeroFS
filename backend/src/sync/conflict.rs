use chrono::Utc;

pub struct ConflictResolver;

impl ConflictResolver {
    /// Generates a conflict filename for `KeepBoth` strategy: `filename (conflict-YYYYMMDD-HHMMSS).ext`
    pub fn generate_conflict_filename(original_name: &str) -> String {
        let now_tag = Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let path = std::path::Path::new(original_name);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(original_name);
        let extension = path.extension().and_then(|e| e.to_str());

        match extension {
            Some(ext) => format!("{} (conflict-{}).{}", stem, now_tag, ext),
            None => format!("{} (conflict-{})", stem, now_tag),
        }
    }
}
