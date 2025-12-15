use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// Simple struct for resolving paths on Linux/Windows/Mac OS
///
/// This convenience struct looks for a file or directory given its name
/// and a set of search paths. The implementation walks through the
/// search paths in order and stops once the file is found.
pub struct FileResolver {
    paths: Vec<PathBuf>,
}

impl FileResolver {
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let paths = vec![std::env::current_dir().unwrap()];
        Self { paths }
    }

    pub fn append(&mut self, path: &Path) {
        self.paths.push(path.to_path_buf());
    }

    #[must_use]
    pub fn resolve(&self, value: &Path) -> PathBuf {
        for path in &self.paths {
            let combined = path.join(value);
            if combined.exists() {
                return combined;
            }
        }
        value.to_path_buf()
    }

    #[must_use]
    pub const fn paths(&self) -> &Vec<PathBuf> {
        &self.paths
    }
}

pub static FILE_RESOLVER: LazyLock<Mutex<FileResolver>> =
    LazyLock::new(|| Mutex::new(FileResolver::new()));
