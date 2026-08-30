//! Extracted from file_service.rs::list_directory_paged (67.md §5-6)

use crate::domain::{DirectoryListing, SortField, SortOrder};

#[derive(Debug, Clone)]
pub struct ListOptions {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    pub sort: Option<SortField>,
    pub order: Option<SortOrder>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

impl ListOptions {
    pub fn from_legacy(
        path: Option<String>,
        show_hidden: Option<bool>,
        sort: Option<&str>,
        order: Option<&str>,
        cursor: Option<String>,
        limit: Option<usize>,
    ) -> Self {
        let sort_typed = sort.and_then(|s| s.parse::<SortField>().ok());
        let order_typed = order.and_then(|o| o.parse::<SortOrder>().ok());
        Self {
            path,
            show_hidden,
            sort: sort_typed,
            order: order_typed,
            cursor,
            limit,
        }
    }
    pub fn sort_str(&self) -> Option<&'static str> {
        match self.sort {
            Some(SortField::Name) => Some("name"),
            Some(SortField::Size) => Some("size"),
            Some(SortField::Modified) => Some("modified"),
            None => None,
        }
    }
    pub fn order_str(&self) -> Option<&'static str> {
        match self.order {
            Some(SortOrder::Asc) => Some("asc"),
            Some(SortOrder::Desc) => Some("desc"),
            None => None,
        }
    }
    #[allow(dead_code)]
    pub fn limit_or_default(&self, default: usize, max: usize) -> usize {
        self.limit.unwrap_or(default).clamp(1, max)
    }
}

// Placeholder for future paginated listing logic moved from file_service.rs
// For Phase 3, FileService::list_directory_paged delegates here via ListOptions.
pub fn sort_entries_placeholder(_listing: &mut DirectoryListing, _opts: &ListOptions) {
    // sorting moved here; actual implementation lives in file_service.rs until full extraction
}
