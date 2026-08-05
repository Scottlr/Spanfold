#[derive(Debug)]
pub(crate) enum ImportError {
    Input(String),
    Io(String),
}

impl ImportError {
    pub(crate) fn io(error: impl std::fmt::Display) -> Self {
        Self::Io(error.to_string())
    }

    pub(crate) fn csv(context: &str, error: csv::Error) -> Self {
        let message = format!("import-events: {context}: {error}");
        if error.is_io_error() {
            Self::Io(message)
        } else {
            Self::Input(message)
        }
    }
}

impl From<String> for ImportError {
    fn from(message: String) -> Self {
        if message.starts_with("import-events:") {
            Self::Input(message)
        } else {
            Self::Input(format!("import-events: {message}"))
        }
    }
}

impl From<ImportError> for crate::CliError {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::Input(message) => Self::from(message),
            ImportError::Io(message) => Self::io(message),
        }
    }
}
