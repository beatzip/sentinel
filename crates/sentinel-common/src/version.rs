use serde::{Deserialize, Serialize};

/// Version information for data models.
/// Ensures backward compatibility and reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DataVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl DataVersion {
    pub const CURRENT: Self = Self {
        major: 1,
        minor: 0,
        patch: 0,
    };

    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Check if this version is compatible with the required version
    pub fn is_compatible_with(&self, required: &DataVersion) -> bool {
        self.major == required.major && self.minor >= required.minor
    }
}

impl std::fmt::Display for DataVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_display() {
        let v = DataVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_compatibility() {
        let v1_0 = DataVersion::new(1, 0, 0);
        let v1_1 = DataVersion::new(1, 1, 0);
        let v2_0 = DataVersion::new(2, 0, 0);

        assert!(v1_1.is_compatible_with(&v1_0));
        assert!(!v1_0.is_compatible_with(&v1_1));
        assert!(!v1_0.is_compatible_with(&v2_0));
    }
}
