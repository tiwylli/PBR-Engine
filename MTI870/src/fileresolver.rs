use std::path::{Path, PathBuf};
use std::sync::Mutex;

use lazy_static::lazy_static;

/// Simple struct for resolving paths on Linux/Windows/Mac OS
///
/// This convenience struct looks for a file or directory given its name
/// and a set of search paths. The implementation walks through the
/// search paths in order and stops once the file is found.
pub struct FileResolver {
    paths: Vec<PathBuf>,
}

impl FileResolver {
    pub fn new() -> Self {
        let mut paths = Vec::new();
        paths.push(std::env::current_dir().unwrap());
        FileResolver { paths }
    }

    pub fn append(&mut self, path: &Path) {
        self.paths.push(path.to_path_buf());
    }

    pub fn resolve(&self, value: &Path) -> PathBuf {
        for path in &self.paths {
            let combined = path.join(value);
            if combined.exists() {
                return combined;
            }
        }
        value.to_path_buf()
    }

    pub fn paths(&self) -> &Vec<PathBuf> {
        &self.paths
    }
}

lazy_static! {
    pub static ref FILE_RESOLVER: Mutex<FileResolver> = Mutex::new(FileResolver::new());
}
