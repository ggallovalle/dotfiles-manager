use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobBuilder, GlobSetBuilder};
use ignore::{self, Walk, WalkBuilder};

pub struct FileTransferBuilder {
    sources: Vec<(PathBuf, Option<String>)>,
    dry_run: bool,
    target: PathBuf,
    force: bool,
    action: FileTransferAction,
}

impl FileTransferBuilder {
    pub fn new<S: AsRef<str>>(source: S, target: S) -> Self {
        FileTransferBuilder {
            dry_run: false,
            force: false,
            action: FileTransferAction::Copy,
            sources: vec![Self::split_source_and_pattern(source.as_ref())],
            target: PathBuf::from(target.as_ref()),
        }
    }

    pub fn build(&self) -> FileTransfer {
        let first_source = &self.sources[0];
        let bases: Vec<PathBuf> = self.sources.iter().map(|(base, _)| base.clone()).collect();
        let target = self.target.clone();

        let mut walk_builder = WalkBuilder::new(bases[0].clone());
        walk_builder.hidden(false);
        walk_builder.follow_links(false);
        let mut glob_builder = None;
        Self::add_glob(&mut glob_builder, first_source.1.clone());
        for others_sources in &self.sources[1..] {
            walk_builder.add(others_sources.0.clone());
            Self::add_glob(&mut glob_builder, others_sources.1.clone());
        }

        if let Some(glob_builder) = glob_builder
            && let Ok(glob_set) = glob_builder.build()
        {
            walk_builder.filter_entry({
                let bases = bases.clone();
                let glob_set = glob_set;
                move |entry| {
                match entry.file_type() {
                    Some(ft) if ft.is_dir() => return true, // always include directories
                    None => return false,                   // include if we can't determine the type
                    _ => {}
                }
                
                let base = bases.iter().find(|base| entry.path().starts_with(base)).unwrap();
                let rel_path = entry.path().strip_prefix(&base).unwrap();
                let is_match = glob_set.is_match(rel_path);
                tracing::info!(is_match = is_match, rel_path = %rel_path.display(), entry = ?entry, "matching_glob");
                is_match
            }});
        }

        FileTransfer {
            walk_builder: walk_builder,
            target: self.target.clone(),
            dry_run: self.dry_run,
            force: self.force,
            action: self.action,
            bases: bases.clone(),
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

    pub fn add_source(mut self, source: &str) -> Self {
        self.sources.push(Self::split_source_and_pattern(source));
        self
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

    fn add_glob(builder: &mut Option<GlobSetBuilder>, pattern: Option<String>) {
        if let Some(pattern) = pattern {
            if let Ok(glob) = Glob::new(&pattern) {
                if builder.is_none() {
                    *builder = Some(GlobSetBuilder::new());
                }
                builder.as_mut().unwrap().add(glob);
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
    bases: Vec<PathBuf>,
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
            bases: self.bases.clone(),
        }
    }
}

pub struct FileTransferIterator {
    inner: Walk,
    target: PathBuf,
    dry_run: bool,
    force: bool,
    action: FileTransferAction,
    bases: Vec<PathBuf>,
}

impl FileTransferIterator {
    pub fn get_target_for(&self, entry: &ignore::DirEntry) -> PathBuf {
        let base = self.bases.iter().find(|base| entry.path().starts_with(base)).unwrap();
        let rel_path = entry.path().strip_prefix(base).unwrap();
        self.target.join(rel_path)
    }
}

impl Iterator for FileTransferIterator {
    type Item = Result<ignore::DirEntry, ignore::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(res) = self.inner.next() {
            match res {
                // for some reasons this is stdin, wtf ignore it
                Ok(entry) if entry.file_type().is_none() => continue,
                Ok(entry) if entry.file_type().unwrap().is_dir() => {
                    let target = self.get_target_for(&entry);

                    // ensure they exist in the target
                    tracing::info!(dry_run = self.dry_run, target = %target.display(), "ensuring directory exists");
                    if !self.dry_run {
                        match std::fs::create_dir_all(target) {
                            Ok(_) => {}
                            Err(e) => {
                                return Some(Err(e.into()));
                            }
                        }
                    }
                    return Some(Ok(entry));
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
                        Ok(_) => return Some(Ok(entry)),
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
        let transfer =
            // FileTransfer::builder("/home/kbroom/dotfiles/awesome/config", "/home/kbroom/.config")
            // FileTransfer::builder("/home/kbroom/dotfiles/awesome/config/**/*.lua", "/home/kbroom/.config")
            FileTransfer::builder("/home/kbroom/dotfiles/awesome/config/awesome/*.lua", "/home/kbroom/.config/test")
            .add_source("/home/kbroom/dotfiles/git/*.yaml")
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
                    tracing::info!("Processed: {}", entry.path().display());
                }
                Err(e) => {
                    tracing::error!("Error: {:?}", e);
                }
            }
        }

        assert!(false);
    }
}
