use crate::domain::{SortField, SortOrder};

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort: Option<SortField>,
    pub order: Option<SortOrder>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}
