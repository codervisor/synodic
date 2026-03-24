pub mod claude;
pub mod copilot;

use crate::events::Event;
use anyhow::Result;
use std::path::Path;

/// Trait for parsing agent session logs into governance events.
pub trait LogParser {
    /// Parse a log file and return detected events.
    fn parse(&self, path: &Path) -> Result<Vec<Event>>;
    /// The source name for events produced by this parser.
    fn source(&self) -> &str;
}
