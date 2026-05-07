use std::{fs, path::Path};

pub fn write_markdown(path: impl AsRef<Path>, content: &str) -> Result<(), std::io::Error> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
