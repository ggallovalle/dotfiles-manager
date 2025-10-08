#![allow(unused)]

use crate::{
    config::{BundleItem, Config, ConfigDiagnostic, ConfigError, OneOf},
    file_transfer::FileOp,
};
use ignore::{WalkBuilder, WalkState};
use indexmap::IndexMap;
use kdl;
use miette;
use std::{
    fmt::Write,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use valuable;

mod config;
mod dir_entry;
mod env;
mod file_transfer;
mod template;
mod walker;

#[derive(Error, Debug, miette::Diagnostic, Clone)]
pub enum DotsError {
    // #[error("io error: {0}")]
    // IO(#[from] std::io::Error),
    // #[error("fmt error: {0}")]
    // Fmt(#[from] std::fmt::Error),
    #[error("config not found: {0}")]
    ConfigNotFound(PathBuf),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Config(#[from] ConfigError),
    #[error("bundle not found: {0}")]
    #[diagnostic()]
    BundleNotFound(String, #[help] String),
}

/// The main crate struct, it contains all needed medata about a
/// dotfile directory and how to install it.
pub struct Dots {
    /// path to self configuration, relative to $HOME
    path: PathBuf,
    bundles: Option<Vec<String>>,
    dry_run: bool,
    force: bool,
    pub config: Config,
}

impl Dots {
    pub fn create(
        path: PathBuf,
        dry_run: bool,
        force: bool,
        bundles: Vec<String>,
    ) -> Result<Self, DotsError> {
        let contents = Arc::new(
            std::fs::read_to_string(&path).map_err(|_| DotsError::ConfigNotFound(path.clone()))?,
        );
        let kdl_doc = kdl::KdlDocument::parse(&contents).map_err(|e| {
            DotsError::Config(ConfigError::from_file(
                &path,
                contents.clone(),
                e.diagnostics.into_iter().map(Into::into).collect(),
            ))
        })?;

        let config = Config::from_kdl(kdl_doc).map_err(|err| {
            DotsError::Config(ConfigError::from_file(&path, contents.clone(), vec![err]))
        })?;
        for bundle in &bundles {
            if !config.bundles.contains_key(bundle) {
                return Err(DotsError::BundleNotFound(
                    bundle.clone(),
                    format!("expected {}", OneOf::from_iter(config.bundles.keys())),
                ));
            }
        }

        Ok(Dots {
            path,
            dry_run,
            force,
            bundles: if bundles.is_empty() { None } else { Some(bundles) },
            config,
        })
    }

    fn public_bundles(&self) -> IndexMap<&str, &Vec<BundleItem>> {
        let mut all: IndexMap<&str, &Vec<BundleItem>> = IndexMap::new();
        if let Some(bundles) = &self.bundles {
            for bundle in bundles {
                let bundle = bundle.as_str();
                let items = self.config.bundles.get(bundle).expect("already checked on create");

                all.insert(bundle, items);
            }
        } else {
            for (bundle_name, bundle_items) in &self.config.bundles {
                all.insert(bundle_name, bundle_items);
            }
        }
        all
    }

    pub fn doctor(&mut self) -> Result<(), DotsError> {
        Ok(())
    }

    pub fn up(&mut self) -> Result<(), DotsError> {
        let mut walk_builder = walker::WalkerBuilder::new();
        let mut op_cp = file_transfer::CopyOp::default();
        op_cp.dry_run(self.dry_run);
        op_cp.force(self.force);
        let mut op_link = file_transfer::LinkOp::default();
        op_link.dry_run(self.dry_run);
        op_link.force(self.force);

        for (bundle_name, items) in self.public_bundles() {
            let span = tracing::span!(
                tracing::Level::DEBUG,
                "bundle",
                bundle.id = bundle_name,
                bundle.op = "dotfiles_install"
            );
            let _span_guard = span.enter();

            for item in items {
                if let BundleItem::Copy { source, target, .. } = item {
                    walk_builder.add_source(source, target, (bundle_name, "cp"));
                } else if let BundleItem::Link { source, target, .. } = item {
                    walk_builder.add_source(source, target, (bundle_name, "ln"));
                }
            }
        }

        // NOTE:  0.11s user 0.08s system 103% cpu 0.183 total
        // NOTE: with planner 0.11s user 0.07s system 103% cpu 0.179 total
        // NOTE: with copy cargo run -pdots -Fcli -q -- -vvv -c dotfiles.all.kdl dotfiles install  0.24s user 0.15s system 103% cpu 0.376 total
        for entry in walk_builder.build() {
            let source = entry.path();
            let depth = entry.depth();
            let file_type = if entry.is_dir() { "dir" } else { "file" };
            let destination = entry.destination();
            let (bundle_name, op) = entry.meta().data;
            match op {
                "cp" => {
                    let cp_result = op_cp.apply(&entry);
                    tracing::info!(src = %source.display(), dst = %destination.display(), depth = depth, file_type = file_type, bundle = bundle_name, result = %cp_result, "cp" );
                }
                "ln" => {
                    let ln_result = op_link.apply(&entry);
                    tracing::info!(src = %source.display(), dst = %destination.display(), depth = depth, file_type = file_type, bundle = bundle_name, result = %ln_result, "ln" );
                }
                _ => {}
            }
        }

        let parallel = false;

        // builder.build_parallel().run(|| {
        //     Box::new(|result| {
        //         if let Ok(entry) = result {
        //             if entry.file_type().is_none() {
        //                 return ignore::WalkState::Continue;
        //             }
        //             let source = entry.path();
        //             let depth = entry.depth();
        //             let file_type =
        //                 if entry.file_type().unwrap().is_dir() { "dir" } else { "file" };
        //             tracing::info!(
        //                 source = %source.display(),
        //                 depth = depth,
        //                 file_type = tracing::field::debug(file_type),
        //                 "cp or ln"
        //             );
        //         }
        //         ignore::WalkState::Continue
        //     })
        // });
        // NOTE: 0.11s user 0.09s system 102% cpu 0.188 total

        Ok(())
    }

    pub fn down(&mut self) -> Result<(), DotsError> {
        let mut walk_builder = walker::WalkerBuilder::new();
        let mut op_rm = file_transfer::RemoveOp::default();
        op_rm.dry_run(self.dry_run);
        op_rm.force(self.force);

        for (bundle_name, items) in self.public_bundles() {
            for item in items {
                match item {
                    BundleItem::Copy { source, target, span } => {
                        walk_builder.add_source(source, target, (bundle_name, "rm"));
                    }
                    BundleItem::Link { source, target, span } => {
                        walk_builder.add_source(source, target, (bundle_name, "rm"));
                    }
                    _ => { /* ignore other items */ }
                }
            }
        }

        // NOTE: with rm cargo run -pdots -Fcli -q -- -vvv -c dotfiles.all.kdl dotfiles uninstall  0.22s user 0.13s system 102% cpu 0.340 total
        for entry in walk_builder.build() {
            let source = entry.path();
            let depth = entry.depth();
            let file_type = if entry.is_dir() { "dir" } else { "file" };
            let destination = entry.destination();
            let (bundle_name, op) = entry.meta().data;
            match op {
                "rm" => {
                    if entry.is_dir() {
                        continue;
                    }
                    let rm_result = op_rm.apply(&entry);
                    tracing::info!(src = %source.display(), dst = %destination.display(), depth = depth, file_type = file_type, bundle = bundle_name, result = %rm_result, "rm" );
                }
                _ => {}
            }
        }

        Ok(())
    }
}
