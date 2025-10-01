#![allow(unused)]

use crate::{
    file_transfer::{FileTransfer, FileTransferAction},
    package_manager::ManagerIdentifier,
    settings::{BundleItem, Settings},
    settings_error::{OneOf, SettingsDiagnostic, SettingsError},
};
use indexmap::IndexMap;
use kdl;
use miette;
use std::{fmt::Write, path::PathBuf, sync::Arc};
use thiserror::Error;
use valuable;

mod env;
mod file_transfer;
mod kdl_helpers;
mod package_manager;
mod settings;
mod settings_error;

#[derive(Error, Debug, miette::Diagnostic, Clone)]
pub enum DotsError {
    // #[error("io error: {0}")]
    // IO(#[from] std::io::Error),
    #[error("fmt error: {0}")]
    Fmt(#[from] std::fmt::Error),
    #[error("config not found: {0}")]
    ConfigNotFound(PathBuf),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Settings(#[from] crate::settings_error::SettingsError),
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
    logs: String,
    dry_run: bool,
    pub config: Settings,
}

impl Dots {
    pub fn create(path: PathBuf, dry_run: bool, bundles: Vec<String>) -> Result<Self, DotsError> {
        let contents = Arc::new(
            std::fs::read_to_string(&path).map_err(|_| DotsError::ConfigNotFound(path.clone()))?,
        );
        let kdl_doc = kdl::KdlDocument::parse(&contents).map_err(|e| {
            DotsError::Settings(SettingsError::from_file(
                &path,
                contents.clone(),
                e.diagnostics.into_iter().map(Into::into).collect(),
            ))
        })?;

        let config = Settings::from_kdl(kdl_doc).map_err(|err| {
            DotsError::Settings(SettingsError::from_file(&path, contents.clone(), vec![err]))
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
            logs: String::new(),
            dry_run,
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

    fn log(&mut self, msg: String) -> Result<(), DotsError> {
        if self.dry_run {
            println!("{}", msg);
        }
        writeln!(self.logs, "{}", msg)?;
        Ok(())
    }

    fn install(&mut self, name: &str, manager: &ManagerIdentifier) -> Result<(), DotsError> {
        tracing::info!("installing {} with {}", name, manager);
        match manager {
            ManagerIdentifier::ArchPacman => self.log(format!("pacman -S {}", name))?,
            ManagerIdentifier::ArchYay => self.log(format!("yay -S {}", name))?,
            ManagerIdentifier::ArchParu => self.log(format!("paru -S {}", name))?,
            // ManagerIdentifier::DebianApt => self.log(format!("apt install {}", name))?,
            // ManagerIdentifier::MacBrew => self.log(format!("brew install {}", name))?,
            // ManagerIdentifier::WindowsChoco => self.log(format!("choco install {}", name))?,
            // ManagerIdentifier::WindowsWinget => self.log(format!("winget install {}", name))?,
            // ManagerIdentifier::RustCargo => self.log(format!("cargo install {}", name))?,
        }
        Ok(())
    }

    pub fn dependencies_doctor(&mut self) -> Result<(), DotsError> {
        self.log("Checking dependencies...".to_string())?;
        Ok(())
    }

    pub fn dependencies_install(&mut self) -> Result<(), DotsError> {
        self.log("Installing dependencies...".to_string())?;

        self.install("zsh", &ManagerIdentifier::ArchPacman)?;
        self.install("git", &ManagerIdentifier::ArchPacman)?;
        // self.install("tealdeer", &PackageManager::RustCargo)?;

        Ok(())
    }

    pub fn dependencies_uninstall(&mut self) -> Result<(), DotsError> {
        self.log("Uninstalling dependencies...".to_string())?;
        Ok(())
    }

    pub fn dotfiles_doctor(&mut self) -> Result<(), DotsError> {
        self.log("Checking dotfiles...".to_string())?;
        Ok(())
    }

    pub fn dotfiles_install(&mut self) -> Result<(), DotsError> {
        self.log("Installing dotfiles...".to_string())?;
        for (name, items) in self.public_bundles() {
            let span = tracing::span!(
                tracing::Level::DEBUG,
                "bundle",
                bundle.id = name,
                bundle.op = "dotfiles_install"
            );
            let _span_guard = span.enter();
            for item in items {
                if let BundleItem::Copy { source, target, span, recursive } = item {
                    tracing::info!(source = %source.display(), target = %target.display(), recursive = recursive, "bundle_item_copy");
                    if *recursive {
                        for copy_log in FileTransfer::builder(source, target)
                            .dry_run(self.dry_run)
                            .action(FileTransferAction::Copy)
                            .build()
                            .iter()
                        {
                        }
                        {}
                    } else {
                        file_transfer::apply_action(
                            &FileTransferAction::Copy,
                            source,
                            target,
                            self.dry_run,
                            false,
                        );
                    }
                } else if let BundleItem::Link { source, target, span } = item {
                    let is_recursive = source.is_dir();
                    tracing::info!(source = %source.display(), target = %target.display(), "bundle_item_link");
                }
            }
        }

        Ok(())
    }

    pub fn dotfiles_uninstall(&mut self) -> Result<(), DotsError> {
        self.log("Uninstalling dotfiles...".to_string())?;
        Ok(())
    }
}
