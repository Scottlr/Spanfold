use super::{
    history::WindowHistory,
    model::{
        ClosedWindow, OpenWindow, WindowHistoryFixtureError, WindowMetadataError, WindowRecordId,
        WindowSegment, WindowTag,
    },
};
use crate::{PrimitiveValue, TemporalPoint, TemporalRange};

#[derive(Clone, Debug, Default)]
/// Fixture-oriented builder for compact histories.
pub struct WindowHistoryFixture {
    history: WindowHistory,
    next_record_id: u64,
}

impl WindowHistoryFixture {
    /// Creates a fixture builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            history: WindowHistory::new(),
            next_record_id: 0,
        }
    }

    /// Adds a closed processing-position window.
    pub fn closed_window(
        mut self,
        window_name: impl Into<String>,
        key: impl Into<String>,
        start_position: i64,
        end_position: i64,
        configure: impl FnOnce(WindowHistoryFixtureWindow) -> WindowHistoryFixtureWindow,
    ) -> Result<Self, WindowHistoryFixtureError> {
        let metadata = configure(WindowHistoryFixtureWindow::default());
        if let Some(error) = metadata.error {
            return Err(error.into());
        }
        let id = self.next_id();
        let range = TemporalRange::positions(start_position, end_position)?;
        self.history.push_closed(ClosedWindow {
            id,
            window_name: window_name.into(),
            key: key.into(),
            range,
            known_at: metadata.known_at,
            source: metadata.source,
            partition: metadata.partition,
            segments: metadata.segments,
            tags: metadata.tags,
            boundary_reason: None,
            boundary_changes: Vec::new(),
        });
        Ok(self)
    }

    /// Adds an open processing-position window.
    pub fn open_window(
        mut self,
        window_name: impl Into<String>,
        key: impl Into<String>,
        start_position: i64,
        configure: impl FnOnce(WindowHistoryFixtureWindow) -> WindowHistoryFixtureWindow,
    ) -> Result<Self, WindowHistoryFixtureError> {
        let metadata = configure(WindowHistoryFixtureWindow::default());
        if let Some(error) = metadata.error {
            return Err(error.into());
        }
        let id = self.next_id();
        self.history.push_open(OpenWindow {
            id,
            window_name: window_name.into(),
            key: key.into(),
            start: TemporalPoint::position(start_position),
            known_at: metadata.known_at,
            source: metadata.source,
            partition: metadata.partition,
            segments: metadata.segments,
            tags: metadata.tags,
        });
        Ok(self)
    }

    /// Builds the history.
    #[must_use]
    pub fn build(self) -> WindowHistory {
        self.history
    }

    fn next_id(&mut self) -> WindowRecordId {
        let id = WindowRecordId::generated(format!("window-{:04}", self.next_record_id));
        self.next_record_id += 1;
        id
    }
}

/// Metadata builder for one fixture window.
#[derive(Clone, Debug, Default)]
pub struct WindowHistoryFixtureWindow {
    known_at: Option<TemporalPoint>,
    source: Option<String>,
    partition: Option<String>,
    segments: Vec<WindowSegment>,
    tags: Vec<WindowTag>,
    error: Option<WindowMetadataError>,
}

impl WindowHistoryFixtureWindow {
    /// Sets the source/lane.
    #[must_use]
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Sets the known-at processing position for the window.
    #[must_use]
    pub fn known_at_position(mut self, position: i64) -> Self {
        self.known_at = Some(TemporalPoint::position(position));
        self
    }

    /// Sets the partition.
    #[must_use]
    pub fn partition(mut self, partition: impl Into<String>) -> Self {
        self.partition = Some(partition.into());
        self
    }

    /// Adds a segment.
    #[must_use]
    pub fn segment(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        match WindowSegment::new(name, value) {
            Ok(segment) => self.segments.push(segment),
            Err(error) if self.error.is_none() => self.error = Some(error),
            Err(_) => {}
        }
        self
    }

    /// Adds a segment with parent metadata.
    #[must_use]
    pub fn child_segment(
        mut self,
        name: impl Into<String>,
        value: impl Into<PrimitiveValue>,
        parent_name: impl Into<String>,
    ) -> Self {
        match WindowSegment::new(name, value).and_then(|segment| segment.with_parent(parent_name)) {
            Ok(segment) => self.segments.push(segment),
            Err(error) if self.error.is_none() => self.error = Some(error),
            Err(_) => {}
        }
        self
    }

    /// Adds a tag.
    #[must_use]
    pub fn tag(mut self, name: impl Into<String>, value: impl Into<PrimitiveValue>) -> Self {
        match WindowTag::new(name, value) {
            Ok(tag) => self.tags.push(tag),
            Err(error) if self.error.is_none() => self.error = Some(error),
            Err(_) => {}
        }
        self
    }
}
