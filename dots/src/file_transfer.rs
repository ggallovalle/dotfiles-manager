use std::collections::VecDeque;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::dir_entry::DirEntry;

pub trait FileOp<T> {
    /// The type of value produced for each visited entry.
    type Output;

    /// Apply the operation to a single entry.
    fn apply(&self, entry: &DirEntry<T>) -> Self::Output;
}

pub struct CopyOp {
    pub dry_run: bool,
    pub force: bool,
}

impl Default for CopyOp {
    fn default() -> Self {
        Self { dry_run: false, force: false }
    }
}

impl CopyOp {
    pub fn dry_run(&mut self, yes: bool) -> &mut Self {
        self.dry_run = yes;
        self
    }

    pub fn force(&mut self, yes: bool) -> &mut Self {
        self.force = yes;
        self
    }

    pub fn copy(&self, src: &Path, dst: &Path) -> CopyOpResult {
        if self.dry_run {
            tracing::trace!(src = %src.display(), dst = %dst.display(), "dry run copy");
            return CopyOpResult::DryRun;
        }

        match (self.force, dst.exists()) {
            (false, true) => {
                tracing::trace!(src = %src.display(), dst = %dst.display(), "skipping existing file");
                CopyOpResult::SkippedExisting
            }
            (true, true) => {
                tracing::trace!(src = %src.display(), dst = %dst.display(), "removing existing file due to force");
                match fs::remove_file(dst) {
                    Err(e) => {
                        tracing::trace!(src = %src.display(), dst = %dst.display(), error = %e, "error removing existing file due to force");
                        e.into()
                    }
                    Ok(_) => Self::action(src, dst),
                }
            }
            (_, false) => Self::action(src, dst),
        }
    }

    fn action(src: &Path, dst: &Path) -> CopyOpResult {
        match fs::copy(src, dst) {
            Err(e) => {
                tracing::error!(src = %src.display(), dst = %dst.display(), error = %e, "copy failed");
                e.into()
            }
            Ok(n) => {
                tracing::info!(src = %src.display(), dst = %dst.display(), bytes = n, "copied");
                CopyOpResult::CopiedForced(n)
            }
        }
    }

    fn ensure_dir(&self, src: &Path, dst: &Path) -> CopyOpResult {
        match fs::create_dir_all(dst) {
            Err(e) => {
                tracing::error!(src = %src.display(), dst = %dst.display(), error = %e, "error creating directory");
                e.into()
            }
            Ok(_) => {
                tracing::trace!(src = %src.display(), dst = %dst.display(), "ensured directory exists");
                CopyOpResult::Copied(0)
            }
        }
    }
}

#[derive(Debug)]
pub enum CopyOpResult {
    Copied(u64),
    CopiedForced(u64),
    SkippedExisting,
    DryRun,
    Error(io::Error),
}

impl Display for CopyOpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopyOpResult::Copied(n) => write!(f, "copied {} bytes", n),
            CopyOpResult::CopiedForced(n) => write!(f, "copied {} bytes (forced)", n),
            CopyOpResult::SkippedExisting => write!(f, "skipped existing file"),
            CopyOpResult::DryRun => write!(f, "dry run, no action taken"),
            CopyOpResult::Error(e) => write!(f, "error: {}", e),
        }
    }
}

impl From<io::Error> for CopyOpResult {
    fn from(e: io::Error) -> Self {
        CopyOpResult::Error(e)
    }
}

impl<T> FileOp<T> for CopyOp {
    type Output = CopyOpResult;

    fn apply(&self, entry: &DirEntry<T>) -> Self::Output {
        let src = entry.path();
        let dst = entry.destination();

        if entry.is_dir() { self.ensure_dir(src, dst) } else { self.copy(src, dst) }
    }
}

pub struct RemoveOp {
    pub dry_run: bool,
    pub force: bool,
}

impl Default for RemoveOp {
    fn default() -> Self {
        Self { dry_run: false, force: false }
    }
}

impl RemoveOp {
    pub fn dry_run(&mut self, yes: bool) -> &mut Self {
        self.dry_run = yes;
        self
    }

    pub fn force(&mut self, yes: bool) -> &mut Self {
        self.force = yes;
        self
    }

    pub fn remove(&self, path: &Path) -> RemoveOpResult {
        if self.dry_run {
            tracing::trace!(path = %path.display(), "dry run remove");
            return RemoveOpResult::DryRun;
        }

        match path.metadata() {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::trace!(path = %path.display(), "skipping non-existing file");
                RemoveOpResult::SkippedNotFound
            }
            Err(e) => {
                tracing::error!(path = %path.display(), error = %e, "error getting metadata");
                e.into()
            }
            Ok(m) if m.is_dir() => match fs::remove_dir_all(path) {
                Err(e) => {
                    tracing::error!(path = %path.display(), error = %e, "error removing directory");
                    e.into()
                }
                Ok(_) => {
                    tracing::info!(path = %path.display(), "removed directory");
                    RemoveOpResult::Removed
                }
            },
            Ok(_) => match fs::remove_file(path) {
                Err(e) => {
                    tracing::error!(path = %path.display(), error = %e, "error removing file");
                    e.into()
                }
                Ok(_) => {
                    tracing::info!(path = %path.display(), "removed file");
                    RemoveOpResult::Removed
                }
            },
        }
    }
}

#[derive(Debug)]
pub enum RemoveOpResult {
    Removed,
    SkippedNotFound,
    DryRun,
    Error(io::Error),
}

impl Display for RemoveOpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoveOpResult::Removed => write!(f, "removed"),
            RemoveOpResult::SkippedNotFound => write!(f, "skipped non-existing file"),
            RemoveOpResult::DryRun => write!(f, "dry run, no action taken"),
            RemoveOpResult::Error(e) => write!(f, "error: {}", e),
        }
    }
}

impl From<io::Error> for RemoveOpResult {
    fn from(e: io::Error) -> Self {
        RemoveOpResult::Error(e)
    }
}

impl<T> FileOp<T> for RemoveOp {
    type Output = RemoveOpResult;

    fn apply(&self, entry: &DirEntry<T>) -> Self::Output {
        let path = entry.destination();
        self.remove(path)
    }
}

#[derive(Debug)]
pub struct LinkOp {
    pub dry_run: bool,
    pub force: bool,
    pub symlink: bool,
}

impl Default for LinkOp {
    fn default() -> Self {
        Self { dry_run: false, force: false, symlink: true }
    }
}

impl LinkOp {
    pub fn dry_run(&mut self, yes: bool) -> &mut Self {
        self.dry_run = yes;
        self
    }

    pub fn force(&mut self, yes: bool) -> &mut Self {
        self.force = yes;
        self
    }

    pub fn is_symlink(&mut self, yes: bool) -> &mut Self {
        self.symlink = yes;
        self
    }

    pub fn is_hardlink(&mut self, yes: bool) -> &mut Self {
        self.symlink = !yes;
        self
    }

    pub fn link(&self, src: &Path, dst: &Path) -> LinkOpResult {
        if self.dry_run {
            tracing::trace!(src = %src.display(), dst = %dst.display(), "dry run link");
            return LinkOpResult::DryRun;
        }

        match (self.force, dst.exists()) {
            (false, true) => {
                tracing::trace!(src = %src.display(), dst = %dst.display(), "skipping existing file");
                LinkOpResult::SkippedExisting
            }
            (true, true) => {
                tracing::trace!(src = %src.display(), dst = %dst.display(), "removing existing file due to force");
                match fs::remove_file(dst) {
                    Err(e) => {
                        tracing::trace!(src = %src.display(), dst = %dst.display(), error = %e, "error removing existing file due to force");
                        e.into()
                    }
                    Ok(_) => Self::action(src, dst, self.symlink),
                }
            }
            (_, false) => Self::action(src, dst, self.symlink),
        }
    }

    pub fn symlink(src: &Path, dst: &Path) -> LinkOpResult {
        Self::action(src, dst, true)
    }

    pub fn hardlink(src: &Path, dst: &Path) -> LinkOpResult {
        Self::action(src, dst, false)
    }

    fn action(src: &Path, dst: &Path, symlink: bool) -> LinkOpResult {
        let result = if symlink { make_symlink(src, dst) } else { fs::hard_link(src, dst) };
        match result {
            Err(e) => {
                tracing::error!(src = %src.display(), dst = %dst.display(), error = %e, "link failed");
                e.into()
            }
            Ok(_) => {
                if symlink {
                    tracing::info!(src = %src.display(), dst = %dst.display(), "symlinked");
                } else {
                    tracing::info!(src = %src.display(), dst = %dst.display(), "hardlinked");
                }
                LinkOpResult::Linked { symlink, hardlink: !symlink }
            }
        }
    }

    fn ensure_dir(&self, src: &Path, dst: &Path) -> LinkOpResult {
        match fs::create_dir_all(dst) {
            Err(e) => {
                tracing::error!(src = %src.display(), dst = %dst.display(), error = %e, "error creating directory");
                e.into()
            }
            Ok(_) => {
                tracing::trace!(src = %src.display(), dst = %dst.display(), "ensured directory exists");
                LinkOpResult::Linked { symlink: false, hardlink: false }
            }
        }
    }
}

#[derive(Debug)]
pub enum LinkOpResult {
    Linked { symlink: bool, hardlink: bool },
    SkippedExisting,
    DryRun,
    Error(io::Error),
}

impl Display for LinkOpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkOpResult::Linked { symlink, hardlink } => {
                if *symlink {
                    write!(f, "symlinked")
                } else {
                    write!(f, "hardlinked")
                }
            }
            LinkOpResult::SkippedExisting => write!(f, "skipped existing file"),
            LinkOpResult::DryRun => write!(f, "dry run, no action taken"),
            LinkOpResult::Error(e) => write!(f, "error: {}", e),
        }
    }
}

impl From<io::Error> for LinkOpResult {
    fn from(e: io::Error) -> Self {
        LinkOpResult::Error(e)
    }
}

impl<T> FileOp<T> for LinkOp {
    type Output = LinkOpResult;

    fn apply(&self, entry: &DirEntry<T>) -> Self::Output {
        let src = entry.path();
        let dst = entry.destination();

        if entry.is_dir() { self.ensure_dir(src, dst) } else { self.link(src, dst) }
    }
}

#[cfg(unix)]
fn make_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn make_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, dst)
    } else {
        std::os::windows::fs::symlink_file(src, dst)
    }
}
