use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::dir_entry::{DirEntry, Source};

#[derive(Debug)]
struct WalkerActionPlanner<T> {
    // target: PathBuf,
    sources: Vec<(Arc<Source<T>>, OsString)>, // (root, top-level name)
    current_idx: usize,
}

impl<T> Default for WalkerActionPlanner<T> {
    fn default() -> Self {
        Self { sources: Vec::new(), current_idx: 0 }
    }
}

impl WalkerActionPlanner<()> {
    pub fn new_empty() -> WalkerActionPlanner<()> {
        WalkerActionPlanner::default()
    }
}

impl<T> WalkerActionPlanner<T> {
    fn add_source(&mut self, source: Source<T>) {
        let name = source.path.file_name().expect("source must have a file name").to_os_string();
        self.sources.push((Arc::new(source), name));
    }

    fn add<P: AsRef<Path>>(&mut self, src: P, target: P, data: T) {
        let source = Source {
            path: src.as_ref().to_path_buf(),
            target: target.as_ref().to_path_buf(),
            data,
        };
        self.add_source(source);
    }

    #[inline(always)]
    fn get_dest_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        depth: usize,
    ) -> Option<(Arc<Source<T>>, PathBuf)> {
        if depth == 0 {
            // we’re at a new source root
            // skip copying the root itself but advance source
            if self.current_idx < self.sources.len() {
                self.current_idx += 1;
            }
            // return None; // don’t copy the root itself
        }

        // Current source we’re in
        let (ref source, ref _top_name) = self.sources[self.current_idx - 1];

        if let Ok(rel) = path.as_ref().strip_prefix(&source.path) {
            let target_len = source.target.as_os_str().len();
            let rel_len = rel.as_os_str().len();
            let mut dest = PathBuf::with_capacity(target_len + rel_len + 2);
            dest.push(&source.target);
            // NOTE: trying to push empty OsStr causes dest that is a file to become a dir
            if rel_len > 0 {
                dest.push(rel);
            }
            // dbg!(&source.target, &rel, &dest);
            Some((source.clone(), dest))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct WalkerBuilder<T> {
    planner: WalkerActionPlanner<T>,
}

impl<T> WalkerBuilder<T> {
    pub fn new() -> Self {
        Self { planner: WalkerActionPlanner::default() }
    }

    pub fn add_source<P: AsRef<Path>>(&mut self, src: P, target: P, data: T) -> &mut Self {
        self.planner.add(src, target, data);
        self
    }

    pub fn build(self) -> Walker<T> {
        let mut walker_builder = ignore::WalkBuilder::new(self.planner.sources[0].0.path.clone());
        for source in self.planner.sources.iter().skip(1) {
            walker_builder.add(source.0.path.clone());
        }
        Walker { walk: walker_builder.build(), planner: self.planner }
    }
}

pub struct Walker<T> {
    walk: ignore::Walk,
    planner: WalkerActionPlanner<T>,
}

impl<T> Iterator for Walker<T> {
    type Item = DirEntry<T>;

    fn next(&mut self) -> Option<Self::Item> {
        for result in self.walk.by_ref() {
            match result {
                Ok(dent) if dent.file_type().is_some() => {
                    if let Some((meta, target)) =
                        self.planner.get_dest_path(dent.path(), dent.depth())
                    {
                        return Some(DirEntry::new(dent, target, meta));
                    } else {
                        // entry is outside of any source root
                        continue;
                    }
                }
                Ok(_) => continue, // e.g. symlink with unknown target
                Err(err) => {
                    tracing::error!(error = %err, "error reading directory entry");
                    continue;
                }
            }
        }
        None
    }
}
