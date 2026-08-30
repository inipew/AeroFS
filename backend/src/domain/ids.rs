use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use utoipa::ToSchema;

/// Typed identifiers preventing Stringly-typed confusion (67.md §9, §131-132).

#[derive(Error, Debug)]
pub enum IdError {
    #[error("Identifier must not be empty")]
    Empty,
    #[error("Identifier too long (max 128): {0}")]
    TooLong(String),
    #[error("Identifier contains null byte")]
    NullByte,
    #[error("Identifier contains invalid characters: {0}")]
    InvalidChars(String),
}

// ── Helpers ──
fn validate_id(s: &str) -> Result<(), IdError> {
    if s.is_empty() {
        return Err(IdError::Empty);
    }
    if s.contains('\0') {
        return Err(IdError::NullByte);
    }
    if s.len() > 128 {
        return Err(IdError::TooLong(s.to_string()));
    }
    Ok(())
}

// ── ConnectionId ──
#[derive(Debug, Clone, PartialEq, Eq, Hash, ToSchema)]
#[schema(value_type = String)]
pub struct ConnectionId(pub String);

impl Serialize for ConnectionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ConnectionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl ConnectionId {
    pub const LOCAL: &'static str = "local";

    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let s = value.into();
        validate_id(&s)?;
        Ok(Self(s))
    }

    pub fn local() -> Self {
        Self(Self::LOCAL.to_string())
    }

    pub fn is_local(&self) -> bool {
        self.0 == Self::LOCAL
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ConnectionId> for String {
    fn from(v: ConnectionId) -> Self {
        v.0
    }
}

impl std::str::FromStr for ConnectionId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl TryFrom<String> for ConnectionId {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl AsRef<str> for ConnectionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ── UserId ──
#[derive(Debug, Clone, PartialEq, Eq, Hash, ToSchema)]
#[schema(value_type = String)]
pub struct UserId(pub String);

impl Serialize for UserId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for UserId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl UserId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let s = value.into();
        validate_id(&s)?;
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UserId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ── SessionId ──
#[derive(Debug, Clone, PartialEq, Eq, Hash, ToSchema)]
#[schema(value_type = String)]
pub struct SessionId(pub String);

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(s).map_err(serde::de::Error::custom)
    }
}

impl SessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let s = value.into();
        validate_id(&s)?;
        if s.len() < 8 {
            return Err(IdError::InvalidChars("session id too short".into()));
        }
        Ok(Self(s))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string())
    }
}

impl AsRef<str> for SessionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ── ConnectionScope ──
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionScope {
    Local,
    Remote(ConnectionId),
}

impl ConnectionScope {
    pub fn from_id(id: ConnectionId) -> Self {
        if id.is_local() {
            Self::Local
        } else {
            Self::Remote(id)
        }
    }
    pub fn as_id(&self) -> &str {
        match self {
            Self::Local => ConnectionId::LOCAL,
            Self::Remote(c) => c.as_str(),
        }
    }
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

// ── Sort enums (67.md §129) ──
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    Name,
    Size,
    Modified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl std::str::FromStr for SortField {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "name" => Ok(Self::Name),
            "size" => Ok(Self::Size),
            "modified" | "mtime" => Ok(Self::Modified),
            _ => Err(format!("invalid sort field: {}", s)),
        }
    }
}

impl std::str::FromStr for SortOrder {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "asc" | "ascending" => Ok(Self::Asc),
            "desc" | "descending" => Ok(Self::Desc),
            _ => Err(format!("invalid sort order: {}", s)),
        }
    }
}
