use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSetBuilder};
use ignore::overrides::OverrideBuilder;
use ignore::{self, Walk, WalkBuilder};

pub struct FileTransferBuilder {
    source: PathBuf,
    target: PathBuf,
    overrides: Option<OverrideBuilder>,
    dry_run: bool,
    force: bool,
    action: FileTransferAction,
}

impl FileTransferBuilder {
    pub fn new<S: AsRef<str>>(source: S, target: S) -> Self {
        FileTransferBuilder {
            dry_run: false,
            force: false,
            action: FileTransferAction::Copy,
            overrides: None,
            source: PathBuf::from(source.as_ref()),
            target: PathBuf::from(target.as_ref()),
        }
    }

    pub fn build(&self) -> FileTransfer {
        let target = self.target.clone();

        let mut walk_builder = WalkBuilder::new(&self.source);
        walk_builder.hidden(false);
        walk_builder.follow_links(false);
        if let Some(glob) = &self.overrides {
            walk_builder.overrides(
                glob.build().expect("this cannot fail, it should fail earlier at 'add_override'"),
            );
        }

        FileTransfer {
            walk_builder: walk_builder,
            target: self.target.clone(),
            dry_run: self.dry_run,
            force: self.force,
            action: self.action,
        }
    }

    pub fn dry_run(mut self, yes: bool) -> Self {
        self.dry_run = yes;
        self
    }

    pub fn force(mut self, yes: bool) -> Self {
        self.force = yes;
        self
    }

    pub fn action(mut self, action: FileTransferAction) -> Self {
        self.action = action;
        self
    }

    pub fn add_override<S: AsRef<str>>(mut self, glob: S) -> Result<Self, ignore::Error> {
        if self.overrides.is_none() {
            self.overrides = Some(OverrideBuilder::new(&self.source));
        }
        match self.overrides.as_mut().unwrap().add(glob.as_ref()) {
            Err(e) => Err(e),
            Ok(_) => Ok(self),
        }
    }

    fn split_source_and_pattern(source: &str) -> (PathBuf, Option<String>) {
        let first_glob = source.find(|c: char| c == '*' || c == '?' || c == '[');
        match first_glob {
            None => (PathBuf::from(source), None),
            Some(idx) => {
                let (path, pattern) = source.split_at(idx);
                (PathBuf::from(path), Some(pattern.to_string()))
            }
        }
    }
}

pub struct FileTransfer {
    walk_builder: WalkBuilder,
    dry_run: bool,
    target: PathBuf,
    force: bool,
    action: FileTransferAction,
}

/// The action that will be applied at the end of the pipeline.
#[derive(Debug, Clone, Copy)]
pub enum FileTransferAction {
    Copy,
    HardLink,
    Symlink,
}

impl FileTransfer {
    pub fn builder<S: AsRef<str>>(source: S, target: S) -> FileTransferBuilder {
        FileTransferBuilder::new(source, target)
    }

    pub fn iter(&self) -> FileTransferIterator {
        let walk = self.walk_builder.build();
        FileTransferIterator {
            inner: walk,
            target: self.target.clone(),
            dry_run: self.dry_run,
            force: self.force,
            action: self.action,
        }
    }
}

pub struct FileTransferIterator {
    inner: Walk,
    target: PathBuf,
    dry_run: bool,
    force: bool,
    action: FileTransferAction,
}

impl FileTransferIterator {
    pub fn get_target_for(&self, entry: &ignore::DirEntry) -> PathBuf {
        // let base = self.bases.iter().find(|base| entry.path().starts_with(base)).unwrap();
        // let rel_path = entry.path().strip_prefix(base).unwrap();
        // self.target.join(rel_path)

        let depth = entry.depth();
        if depth == 0 {
            return self.target.clone();
        }
        // tracing::debug!(entry = %entry.path().display(), depth = depth, target = %self.target.display(), "getting target for entry");
        let target = self.target.join(path_tail(entry.path(), depth).unwrap());
        target
    }
}

/// Get the last `size` components of a path.
/// - If the path has less than `size` components, return None.
/// - If size is 1, return the file name.
/// - If size is 2, return the parent and file name.
/// - If size is 3, return the grandparent, parent and file name.
/// - If size is greater than 3, return the last `size` components.
///
/// Hot path, only supports up to 3 components without allocation.
fn path_tail(path: &Path, size: usize) -> Option<PathBuf> {
    let mut components = path.components();
    match size {
        0 => None,
        1 => components.next_back().map(|c| c.as_os_str().into()),
        2 => {
            let second = components.next_back();
            let first = components.next_back();
            match (first, second) {
                (Some(f), Some(s)) => {
                    let mut instance = PathBuf::from(f.as_os_str());
                    instance.push(s.as_os_str());
                    Some(instance)
                }
                _ => None,
            }
        }
        3 => {
            let third = components.next_back();
            let second = components.next_back();
            let first = components.next_back();
            match (first, second, third) {
                (Some(f), Some(s), Some(t)) => {
                    let mut instance = PathBuf::from(f.as_os_str());
                    instance.push(s.as_os_str());
                    instance.push(t.as_os_str());
                    Some(instance)
                }
                _ => None,
            }
        }
        n => {
            let mut deque = VecDeque::with_capacity(n);
            let mut counter = 0;
            while let Some(c) = components.next_back()
                && counter < n
            {
                deque.push_front(c.as_os_str());
                counter += 1;
            }
            if counter < n {
                return None;
            }
            let instace = deque.into_iter().collect::<PathBuf>();
            Some(instace)
        }
    }
}

pub struct FileTransferEntry {
    target: PathBuf,
    ignore_entry: ignore::DirEntry,
}

impl FileTransferEntry {
    pub fn source(&self) -> &Path {
        &self.ignore_entry.path()
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    pub fn is_file(&self) -> bool {
        self.ignore_entry
            .file_type()
            .expect("file is always Some because stdin is ignored")
            .is_file()
    }

    pub fn is_dir(&self) -> bool {
        self.ignore_entry
            .file_type()
            .expect("file is always Some because stdin is ignored")
            .is_dir()
    }
}

impl Iterator for FileTransferIterator {
    type Item = Result<FileTransferEntry, ignore::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(res) = self.inner.next() {
            match res {
                // for some reasons this is stdin, wtf ignore it
                Ok(entry) if entry.file_type().is_none() => continue,
                Ok(entry) if entry.file_type().unwrap().is_dir() => {
                    let target = self.get_target_for(&entry);

                    // ensure they exist in the target
                    tracing::info!(dry_run = self.dry_run, target = %target.display(), depth = &entry.depth(),  "ensuring directory exists");
                    if !self.dry_run {
                        match std::fs::create_dir_all(&target) {
                            Ok(_) => {}
                            Err(e) => {
                                return Some(Err(e.into()));
                            }
                        }
                    }
                    return Some(Ok(FileTransferEntry { target: target, ignore_entry: entry }));
                }
                Ok(entry) => {
                    let target = self.get_target_for(&entry);

                    match apply_action(
                        &self.action,
                        entry.path(),
                        &target,
                        self.dry_run,
                        self.force,
                    ) {
                        Ok(_) => {
                            return Some(Ok(FileTransferEntry {
                                target: target,
                                ignore_entry: entry,
                            }));
                        }
                        Err(e) => return Some(Err(e.into())),
                    }
                }
                Err(e) => return Some(Err(e)),
            }
        }
        None
    }
}

fn apply_action(
    action: &FileTransferAction,
    source: &Path,
    target: &Path,
    dry_run: bool,
    force: bool,
) -> io::Result<()> {
    let result = match action {
        FileTransferAction::Copy => {
            tracing::info!(dry_run = dry_run, source = %source.display(), target = %target.display(), "copying");
            if !dry_run {
                if force && target.exists() {
                    tracing::info!(target = %target.display(), "removing existing file due to force");
                    fs::remove_file(target)?;
                }
                fs::copy(source, target).map(|_| ())
            } else {
                Ok(())
            }
        }
        FileTransferAction::HardLink => {
            tracing::info!(dry_run = dry_run, source = %source.display(), target = %target.display(), "hard linking");
            if !dry_run { fs::hard_link(source, target) } else { Ok(()) }
        }
        FileTransferAction::Symlink => {
            tracing::info!(dry_run = dry_run, source = %source.display(), target = %target.display(), "symlinking");
            if !dry_run { make_symlink(source, target) } else { Ok(()) }
        }
    };
    match result {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && force => {
            tracing::info!(target = %target.display(), "removing existing file due to force");
            fs::remove_file(target)?;
            // make_symlink(source, target)
            apply_action(action, source, target, dry_run, force)
        }
        other => other,
    }
    // result
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

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: use https://crates.io/crates/sealed_test
    #[test]
    fn test_dry_run_copy() {
        tracing_subscriber::fmt::init();
        let transfer = FileTransfer::builder(
            "/home/kbroom/dotfiles/awesome/config",
            "/home/kbroom/.config/test",
        )
        .add_override("*.lua")
        .unwrap()
        // FileTransfer::builder("/home/kbroom/dotfiles/awesome/config/**/*.lua", "/home/kbroom/.config")
        // FileTransfer::builder("/home/kbroom/dotfiles/awesome/config/awesome/*.lua", "/home/kbroom/.config/test")
        // FileTransfer::builder("/home/kbroom/dotfiles/awesome/config/awesome/[abc", "/home/kbroom/.config/test") // invalid glob
        // .add_source("/home/kbroom/dotfiles/git/*.yaml")
        // .dry_run(true)
        .force(true)
        .action(FileTransferAction::Copy)
        // .action(FileTransferAction::Symlink)
        // .action(FileTransferAction::HardLink)
        .build();
        // TODO: base should be dynamic to support multiple sources

        for entry in transfer.iter() {
            match entry {
                Ok(entry) => {
                    tracing::info!("Processed: {}", entry.target().display());
                }
                Err(e) => {
                    tracing::error!("Error: {:?}", e);
                }
            }
        }

        assert!(false);
    }
}
