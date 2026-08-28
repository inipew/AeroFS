use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
    pub total_size: u64,
}

impl ByteRange {
    /// Returns the length of the byte chunk (inclusive range: end - start + 1)
    pub fn length(&self) -> u64 {
        if self.end >= self.start {
            self.end - self.start + 1
        } else {
            0
        }
    }

    /// Formats the standard HTTP Content-Range response header (RFC 9110 / RFC 7233)
    pub fn content_range_header(&self) -> String {
        format!("bytes {}-{}/{}", self.start, self.end, self.total_size)
    }

    /// Formats the 416 Range Not Satisfiable Content-Range header
    pub fn unsatisfiable_header(total_size: u64) -> String {
        format!("bytes */{}", total_size)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RangeError {
    #[error("Invalid Range header format: {0}")]
    InvalidFormat(String),

    #[error("Multi-range requests are not supported")]
    MultiRangeNotSupported,

    #[error("Range not satisfiable for file of size {0}")]
    NotSatisfiable(u64),
}

/// Parses a standard RFC 9110 / RFC 7233 Range header into a single validated ByteRange.
///
/// Supported formats:
/// - Bounded: `bytes=0-499`
/// - Open-ended: `bytes=500-`
/// - Suffix: `bytes=-500` (last 500 bytes)
pub fn parse_single_byte_range(
    range_header: &str,
    total_size: u64,
) -> Result<ByteRange, RangeError> {
    let trimmed = range_header.trim();

    if !trimmed.starts_with("bytes=") {
        return Err(RangeError::InvalidFormat(
            "Missing 'bytes=' unit prefix".into(),
        ));
    }

    let spec = &trimmed["bytes=".len()..];

    // Reject multi-range requests with 416 / explicit rejection
    if spec.contains(',') {
        return Err(RangeError::MultiRangeNotSupported);
    }

    // Zero-byte representations cannot satisfy any byte range
    if total_size == 0 {
        return Err(RangeError::NotSatisfiable(0));
    }

    let parts: Vec<&str> = spec.split('-').collect();
    if parts.len() != 2 {
        return Err(RangeError::InvalidFormat(
            "Expected single '-' delimiter".into(),
        ));
    }

    let (p0, p1) = (parts[0].trim(), parts[1].trim());

    if p0.is_empty() && p1.is_empty() {
        return Err(RangeError::InvalidFormat("Empty range bounds".into()));
    }

    if p0.is_empty() {
        // Suffix range: bytes=-500
        let suffix_len: u64 = p1
            .parse()
            .map_err(|_| RangeError::InvalidFormat("Invalid suffix length integer".into()))?;

        if suffix_len == 0 {
            return Err(RangeError::NotSatisfiable(total_size));
        }

        let clamped_suffix = suffix_len.min(total_size);
        let start = total_size - clamped_suffix;
        let end = total_size - 1;

        Ok(ByteRange {
            start,
            end,
            total_size,
        })
    } else if p1.is_empty() {
        // Open-ended range: bytes=500-
        let start: u64 = p0
            .parse()
            .map_err(|_| RangeError::InvalidFormat("Invalid start offset integer".into()))?;

        if start >= total_size {
            return Err(RangeError::NotSatisfiable(total_size));
        }

        let end = total_size - 1;

        Ok(ByteRange {
            start,
            end,
            total_size,
        })
    } else {
        // Bounded range: bytes=100-200
        let start: u64 = p0
            .parse()
            .map_err(|_| RangeError::InvalidFormat("Invalid start offset integer".into()))?;
        let end_spec: u64 = p1
            .parse()
            .map_err(|_| RangeError::InvalidFormat("Invalid end offset integer".into()))?;

        if start > end_spec || start >= total_size {
            return Err(RangeError::NotSatisfiable(total_size));
        }

        let end = end_spec.min(total_size - 1);

        Ok(ByteRange {
            start,
            end,
            total_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bounded_range() {
        let range = parse_single_byte_range("bytes=0-499", 1000).unwrap();
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 499);
        assert_eq!(range.length(), 500);
        assert_eq!(range.content_range_header(), "bytes 0-499/1000");
    }

    #[test]
    fn test_parse_open_ended_range() {
        let range = parse_single_byte_range("bytes=500-", 1000).unwrap();
        assert_eq!(range.start, 500);
        assert_eq!(range.end, 999);
        assert_eq!(range.length(), 500);
    }

    #[test]
    fn test_parse_suffix_range() {
        let range = parse_single_byte_range("bytes=-200", 1000).unwrap();
        assert_eq!(range.start, 800);
        assert_eq!(range.end, 999);
        assert_eq!(range.length(), 200);

        // Suffix larger than file
        let range_large = parse_single_byte_range("bytes=-5000", 1000).unwrap();
        assert_eq!(range_large.start, 0);
        assert_eq!(range_large.end, 999);
        assert_eq!(range_large.length(), 1000);
    }

    #[test]
    fn test_parse_unsatisfiable_ranges() {
        // Start beyond file size
        assert_eq!(
            parse_single_byte_range("bytes=1000-", 1000),
            Err(RangeError::NotSatisfiable(1000))
        );

        // Start > End
        assert_eq!(
            parse_single_byte_range("bytes=500-400", 1000),
            Err(RangeError::NotSatisfiable(1000))
        );

        // Suffix 0
        assert_eq!(
            parse_single_byte_range("bytes=-0", 1000),
            Err(RangeError::NotSatisfiable(1000))
        );

        // 0-byte file
        assert_eq!(
            parse_single_byte_range("bytes=0-0", 0),
            Err(RangeError::NotSatisfiable(0))
        );
    }

    #[test]
    fn test_parse_multi_range_rejected() {
        assert_eq!(
            parse_single_byte_range("bytes=0-100,200-300", 1000),
            Err(RangeError::MultiRangeNotSupported)
        );
    }

    #[test]
    fn test_parse_invalid_formats() {
        assert!(matches!(
            parse_single_byte_range("characters=0-10", 1000),
            Err(RangeError::InvalidFormat(_))
        ));
        assert!(matches!(
            parse_single_byte_range("bytes=abc-def", 1000),
            Err(RangeError::InvalidFormat(_))
        ));
        assert!(matches!(
            parse_single_byte_range("bytes=-", 1000),
            Err(RangeError::InvalidFormat(_))
        ));
    }
}
