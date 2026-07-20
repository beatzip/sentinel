use crate::error::Result;
use std::path::Path;

/// Abstraction for match data sources.
/// This trait allows Sentinel to support different input formats
/// beyond Source 2 .dem files in the future.
pub trait MatchSource {
    /// The type of event produced by this source
    type Event;
    /// The type of metadata about the match
    type Metadata;

    /// Load a match from the given path
    fn load(path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// Get metadata about the match without loading all events
    fn metadata(&self) -> &Self::Metadata;

    /// Iterate over all events in the match
    fn events(&self) -> impl Iterator<Item = Self::Event>;

    /// Get the total number of ticks in the match
    fn tick_count(&self) -> u32;
}
