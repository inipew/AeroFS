#[derive(Debug, Clone)]
pub struct ReadOptions {
    pub path: String,
    pub download: bool,
    pub range: Option<String>,
}
