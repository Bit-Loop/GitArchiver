use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    ReadOnly,
    Operator,
    Admin,
}

impl UserRole {
    pub const CANONICAL_ROLES: [&'static str; 3] = ["admin", "operator", "read_only"];

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "admin" | "administrator" => Some(Self::Admin),
            "operator" | "user" => Some(Self::Operator),
            "read_only" | "readonly" | "viewer" => Some(Self::ReadOnly),
            _ => None,
        }
    }

    pub fn canonical_label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Operator => "operator",
            Self::Admin => "admin",
        }
    }

    pub fn allows(self, required: Self) -> bool {
        self >= required
    }

    pub fn normalize(value: &str) -> Result<&'static str> {
        Self::parse(value)
            .map(Self::canonical_label)
            .ok_or_else(|| anyhow!(Self::validation_message()))
    }

    pub fn validation_message() -> &'static str {
        "Invalid role. Must be one of: admin, operator, read_only (compatibility aliases: user -> operator, viewer -> read_only)"
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical_label())
    }
}

#[cfg(test)]
mod tests {
    use super::UserRole;

    #[test]
    fn parse_supports_legacy_role_aliases() {
        assert_eq!(UserRole::parse("admin"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("administrator"), Some(UserRole::Admin));
        assert_eq!(UserRole::parse("operator"), Some(UserRole::Operator));
        assert_eq!(UserRole::parse("user"), Some(UserRole::Operator));
        assert_eq!(UserRole::parse("read_only"), Some(UserRole::ReadOnly));
        assert_eq!(UserRole::parse("viewer"), Some(UserRole::ReadOnly));
        assert_eq!(UserRole::parse("unknown"), None);
    }

    #[test]
    fn role_hierarchy_allows_expected_access() {
        assert!(UserRole::Admin.allows(UserRole::Operator));
        assert!(UserRole::Operator.allows(UserRole::ReadOnly));
        assert!(!UserRole::ReadOnly.allows(UserRole::Operator));
    }
}
