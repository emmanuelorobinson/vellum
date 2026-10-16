//! Versioned local content. Filesystem sources receive only explicitly authorized, rooted FileSystems.
//! There is no ambient discovery; host settings and permission UI are implemented by later prompts.
mod skills;
mod styles;

pub use skills::{BuiltinSkills, FileSkills, StackedSkills};
pub use styles::{BuiltinStyles, FolderStyles, StackedStyles};

use vellum_core::ports::{ContentError, FileSystem, check_path};

/// Bound each content result well below the IPC response budget. Content is never silently truncated.
pub const MAX_CONTENT_BYTES: usize = 128 * 1024;

fn path(path: &str) -> Result<(), ContentError> {
    check_path(path, false).map_err(|e| ContentError::Invalid(e.to_string()))
}

fn read(fs: &dyn FileSystem, file: &str) -> Result<String, ContentError> {
    let bytes = fs
        .read(file)
        .map_err(|e| ContentError::Unavailable(e.to_string()))?;
    bounded(&bytes, file)
}

fn bounded(bytes: &[u8], name: &str) -> Result<String, ContentError> {
    if bytes.len() > MAX_CONTENT_BYTES {
        return Err(ContentError::Invalid(format!(
            "{name}: exceeds {MAX_CONTENT_BYTES} bytes"
        )));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ContentError::Invalid(format!("{name}: expected UTF-8")))?;
    if text.trim().is_empty() {
        return Err(ContentError::Invalid(format!("{name}: empty content")));
    }
    Ok(text.to_owned())
}
