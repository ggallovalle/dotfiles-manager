use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub struct Source<T> {
    pub path: PathBuf,
    pub target: PathBuf,
    pub data: T,
}

#[derive(Debug)]
pub struct DirEntry<T> {
    dent: ignore::DirEntry,
    target: PathBuf,
    meta: Arc<Source<T>>,
}

impl<T> DirEntry<T> {
    pub fn source(&self) -> &Path {
        self.meta.path.as_ref()
    }

    pub fn target(&self) -> &Path {
        self.meta.target.as_ref()
    }

    pub fn meta(&self) -> &Source<T> {
        let meta = self.meta.as_ref();
        meta
    }

    pub fn path(&self) -> &Path {
        self.dent.path()
    }

    pub fn destination(&self) -> &Path {
        &self.target
    }

    pub fn file_type(&self) -> std::fs::FileType {
        self.dent.file_type().unwrap()
    }

    pub fn depth(&self) -> usize {
        self.dent.depth()
    }

    pub fn file_name(&self) -> &std::ffi::OsStr {
        self.dent.file_name()
    }

    pub fn is_file(&self) -> bool {
        self.file_type().is_file()
    }

    pub fn is_dir(&self) -> bool {
        self.file_type().is_dir()
    }

    pub fn metadata(&self) -> std::fs::Metadata {
        self.dent.metadata().unwrap()
    }
}

impl<T> DirEntry<T> {
    pub(crate) fn new(dent: ignore::DirEntry, target: PathBuf, meta: Arc<Source<T>>) -> Self {
        Self { dent, target, meta }
    }
}
