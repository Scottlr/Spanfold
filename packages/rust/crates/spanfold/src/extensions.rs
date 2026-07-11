//! Portable metadata descriptors for integrations that annotate comparison
//! results. This module deliberately does not execute extension code: callers
//! own registration and behavior, while Spanfold validates and serializes the
//! descriptor/metadata contract.

use serde::Serialize;
use std::collections::BTreeSet;
use thiserror::Error;

/// Selector descriptor exposed by a comparison extension.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonExtensionSelector {
    /// Stable selector name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
}

/// Comparator descriptor exposed by a comparison extension.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonExtensionComparator {
    /// Stable comparator declaration.
    pub declaration: String,
    /// Human-readable description.
    pub description: String,
}

/// Immutable comparison extension descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonExtensionDescriptor {
    /// Stable extension identifier.
    pub id: String,
    /// Human-readable display name.
    #[serde(rename = "displayName")]
    pub display_name: String,
    /// Exposed selectors.
    pub selectors: Vec<ComparisonExtensionSelector>,
    /// Exposed comparators.
    pub comparators: Vec<ComparisonExtensionComparator>,
    /// Exposed metadata keys.
    #[serde(rename = "metadataKeys")]
    pub metadata_keys: Vec<String>,
}

/// Serializable metadata emitted by a comparison extension.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComparisonExtensionMetadata {
    /// Stable extension identifier.
    #[serde(rename = "extensionId")]
    pub extension_id: String,
    /// Stable metadata key.
    pub key: String,
    /// Serialized metadata payload.
    pub value: String,
}

/// Parsed evidence for one cohort-aligned segment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CohortEvidenceMetadata {
    /// Aligned segment index that emitted the metadata.
    #[serde(rename = "segmentIndex")]
    pub segment_index: usize,
    /// Cohort activity rule.
    pub rule: String,
    /// Required active-member count.
    #[serde(rename = "requiredCount")]
    pub required_count: usize,
    /// Observed active-member count.
    #[serde(rename = "activeCount")]
    pub active_count: usize,
    /// Whether the cohort lane was active.
    #[serde(rename = "isActive")]
    pub is_active: bool,
    /// Active source identities.
    #[serde(rename = "activeSources")]
    pub active_sources: Vec<String>,
    /// Raw metadata payload.
    #[serde(rename = "rawValue")]
    pub raw_value: String,
}

/// Builder for immutable, metadata-only comparison extension descriptors.
#[derive(Clone, Debug)]
pub struct ComparisonExtensionBuilder {
    id: String,
    display_name: String,
    selectors: Vec<ComparisonExtensionSelector>,
    comparators: Vec<ComparisonExtensionComparator>,
    metadata_keys: Vec<String>,
}

/// Error returned when an extension descriptor is incomplete or ambiguous.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ComparisonExtensionBuildError {
    /// The extension ID or display name is empty.
    #[error("extension id and display name are required")]
    MissingIdentity,
    /// A descriptor collection contains a duplicate name.
    #[error("duplicate extension descriptor name '{0}'")]
    DuplicateName(String),
}

impl ComparisonExtensionBuilder {
    /// Creates a builder for one extension descriptor.
    #[must_use]
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            selectors: Vec::new(),
            comparators: Vec::new(),
            metadata_keys: Vec::new(),
        }
    }

    /// Registers a selector descriptor.
    #[must_use]
    pub fn add_selector(mut self, name: impl Into<String>, description: impl Into<String>) -> Self {
        self.selectors.push(ComparisonExtensionSelector {
            name: name.into(),
            description: description.into(),
        });
        self
    }

    /// Registers a comparator descriptor.
    #[must_use]
    pub fn add_comparator(
        mut self,
        declaration: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.comparators.push(ComparisonExtensionComparator {
            declaration: declaration.into(),
            description: description.into(),
        });
        self
    }

    /// Registers a metadata key.
    #[must_use]
    pub fn add_metadata_key(mut self, key: impl Into<String>) -> Self {
        self.metadata_keys.push(key.into());
        self
    }

    /// Builds the immutable descriptor.
    pub fn build(self) -> Result<ComparisonExtensionDescriptor, ComparisonExtensionBuildError> {
        if self.id.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err(ComparisonExtensionBuildError::MissingIdentity);
        }
        for names in [
            self.selectors
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            self.comparators
                .iter()
                .map(|item| item.declaration.as_str())
                .collect::<Vec<_>>(),
            self.metadata_keys
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ] {
            let mut seen = BTreeSet::new();
            for name in names {
                if name.trim().is_empty() || !seen.insert(name) {
                    return Err(ComparisonExtensionBuildError::DuplicateName(
                        name.to_owned(),
                    ));
                }
            }
        }
        Ok(ComparisonExtensionDescriptor {
            id: self.id,
            display_name: self.display_name,
            selectors: self.selectors,
            comparators: self.comparators,
            metadata_keys: self.metadata_keys,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ComparisonExtensionBuildError, ComparisonExtensionBuilder};

    #[test]
    fn rejects_incomplete_or_ambiguous_descriptors() {
        assert_eq!(
            ComparisonExtensionBuilder::new(" ", "display").build(),
            Err(ComparisonExtensionBuildError::MissingIdentity)
        );
        assert_eq!(
            ComparisonExtensionBuilder::new("id", "display")
                .add_selector("selector", "first")
                .add_selector("selector", "duplicate")
                .build(),
            Err(ComparisonExtensionBuildError::DuplicateName(
                "selector".to_owned()
            ))
        );
    }

    #[test]
    fn builds_a_valid_descriptor() {
        let descriptor = ComparisonExtensionBuilder::new("fixture", "Fixture")
            .add_selector("home", "Home side")
            .add_comparator("position", "Position")
            .add_metadata_key("provider")
            .build()
            .expect("valid descriptor");

        assert_eq!(descriptor.id, "fixture");
        assert_eq!(descriptor.selectors.len(), 1);
        assert_eq!(descriptor.comparators.len(), 1);
        assert_eq!(descriptor.metadata_keys, ["provider"]);
    }
}
