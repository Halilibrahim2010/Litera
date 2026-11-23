use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub icon_name: String,
}

impl FileEntry {
    pub fn new(path: PathBuf) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().to_string();
        let symlink_meta = std::fs::symlink_metadata(&path).ok()?;
        let is_symlink = symlink_meta.file_type().is_symlink();
        let metadata = std::fs::metadata(&path).ok().unwrap_or(symlink_meta);
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };
        let modified = metadata.modified().ok();

        let icon_name = if is_dir {
            if is_symlink {
                "folder-symbolic".to_string()
            } else {
                "folder".to_string()
            }
        } else {
            get_icon_for_filename(&name)
        };

        Some(Self {
            name,
            path,
            is_dir,
            is_symlink,
            size,
            modified,
            icon_name,
        })
    }

    pub fn formatted_size(&self) -> String {
        if self.is_dir {
            "--".to_string()
        } else {
            format_bytes(self.size)
        }
    }
}

pub fn list_directory(dir: &Path, show_hidden: bool) -> Vec<FileEntry> {
    let mut entries = Vec::new();

    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for item in read_dir.flatten() {
            let path = item.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            if let Some(entry) = FileEntry::new(path) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| {
        if a.is_dir == b.is_dir {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        } else if a.is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    });

    entries
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn get_icon_for_filename(name: &str) -> String {
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "rs" => "text-x-rust",
        "py" => "text-x-python",
        "c" | "cpp" | "h" | "hpp" => "text-x-c",
        "js" | "ts" | "jsx" | "tsx" => "text-x-javascript",
        "html" | "htm" => "text-html",
        "css" | "scss" | "sass" => "text-css",
        "json" | "toml" | "yaml" | "yml" | "xml" => "text-x-generic",
        "md" | "txt" | "markdown" => "text-x-generic",
        "png" | "jpg" | "jpeg" | "webp" | "svg" | "gif" => "image-x-generic",
        "mp3" | "wav" | "flac" | "ogg" => "audio-x-generic",
        "mp4" | "mkv" | "avi" | "webm" => "video-x-generic",
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" => "package-x-generic",
        "pdf" => "application-pdf",
        "sh" | "bash" | "zsh" => "text-x-script",
        _ => "text-x-generic",
    }
    .to_string()
}
