use std::{fs, io::Write, path::PathBuf};

use super::super::ImportedWindow;
use super::error::ImportError;

pub(crate) trait ImportedWindowSink {
    fn push(&mut self, window: ImportedWindow) -> Result<(), ImportError>;
}

impl ImportedWindowSink for Vec<ImportedWindow> {
    fn push(&mut self, window: ImportedWindow) -> Result<(), ImportError> {
        self.push(window);
        Ok(())
    }
}

pub(crate) struct JsonlWindowSink<W> {
    writer: W,
    output: PathBuf,
}

impl<W: Write> JsonlWindowSink<W> {
    pub(crate) fn new(writer: W, output: PathBuf) -> Self {
        Self { writer, output }
    }
}

impl<W: Write> ImportedWindowSink for JsonlWindowSink<W> {
    fn push(&mut self, window: ImportedWindow) -> Result<(), ImportError> {
        let line = serde_json::to_string(&window).map_err(|error| error.to_string())?;
        writeln!(self.writer, "{line}").map_err(|error| {
            ImportError::io(format!(
                "import-events: write output '{}': {error}",
                self.output.display()
            ))
        })
    }
}

impl JsonlWindowSink<fs::File> {
    pub(crate) fn finish(mut self) -> Result<(), ImportError> {
        self.writer.flush().map_err(|error| {
            ImportError::io(format!(
                "import-events: flush output '{}': {error}",
                self.output.display()
            ))
        })?;
        self.writer.sync_all().map_err(|error| {
            ImportError::io(format!(
                "import-events: sync output '{}': {error}",
                self.output.display()
            ))
        })?;
        Ok(())
    }
}
