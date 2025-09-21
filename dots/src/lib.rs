#![allow(unused)]

use kdl;
use miette;
use std::{fmt::Write, path::PathBuf};
use thiserror::Error;

mod config;
mod settings;
mod settings_error;

use crate::config::root::PackageManager;

#[derive(Error, Debug, miette::Diagnostic)]
pub enum DotsError {
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
    #[error("fmt error: {0}")]
    Fmt(#[from] std::fmt::Error),
    #[error("config not found: {0}")]
    ConfigNotFound(PathBuf),
    #[error(transparent)]
    #[diagnostic(transparent)]
    Settings(#[from] crate::settings_error::SettingsError),
}

/// The main crate struct, it contains all needed medata about a
/// dotfile directory and how to install it.
pub struct Dots {
    /// path to self configuration, relative to $HOME
    path: PathBuf,
    bundles: Option<Vec<String>>,
    logs: String,
    dry_run: bool,
}

enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

impl Dots {
    pub fn create(
        path: PathBuf,
        dry_run: bool,
        bundles: Vec<String>,
        verbosity: u8,
    ) -> Result<Self, DotsError> {
        let the_bundles = if bundles.is_empty() { None } else { Some(bundles) };
        let the_verbosity = match verbosity {
            0 => Verbosity::Quiet,
            1 => Verbosity::Normal,
            _ => Verbosity::Verbose,
        };

        Err(DotsError::Settings(crate::settings_error::SettingsError::from_file(
            // "dotfiles.wsl-archlinux.kdl",
            path.to_string_lossy(),
            std::sync::Arc::new("sudo npm".to_string()),
            vec![
                crate::settings_error::SettingsDiagnostic::unknown_variant(
                    "sudo",
                    &["pacman", "yay", "paru", "apt", "brew", "choco", "winget", "cargo"],
                    (0, 4),
                ),
                crate::settings_error::SettingsDiagnostic::unknown_variant(
                    "npm",
                    &["pacman", "yay", "paru", "apt", "brew", "choco", "winget", "cargo"],
                    (5, 3),
                ),
            ],
        )))

        // let contents =
        //     std::fs::read_to_string(&path).map_err(|_| DotsError::ConfigNotFound(path.clone()))?;
        // let kdl_doc = kdl::KdlDocument::parse(&contents).map_err(|e| {
        //     DotsError::Settings(crate::settings_error::SettingsError::from_file(
        //         path.to_string_lossy(),
        //         std::sync::Arc::new(contents.clone()),
        //         e.diagnostics.into_iter().map(|d| {
        //             crate::settings_error::SettingsDiagnostic::ParseError(d)
        //         }).collect(),
        //     ))
        // })?;

        // Ok(Dots { path, logs: String::new(), dry_run, bundles: the_bundles })
    }

    fn log(&mut self, msg: String) -> Result<(), DotsError> {
        if self.dry_run {
            println!("{}", msg);
        }
        writeln!(self.logs, "{}", msg)?;
        Ok(())
    }

    fn install(&mut self, name: &str, manager: &PackageManager) -> Result<(), DotsError> {
        match manager {
            PackageManager::ArchPacman => self.log(format!("pacman -S {}", name))?,
            PackageManager::ArchYay => self.log(format!("yay -S {}", name))?,
            PackageManager::ArchParu => self.log(format!("paru -S {}", name))?,
            PackageManager::DebianApt => self.log(format!("apt install {}", name))?,
            PackageManager::MacBrew => self.log(format!("brew install {}", name))?,
            PackageManager::WindowsChoco => self.log(format!("choco install {}", name))?,
            PackageManager::WindowsWinget => self.log(format!("winget install {}", name))?,
            PackageManager::RustCargo => self.log(format!("cargo install {}", name))?,
        }
        Ok(())
    }

    pub fn dependencies_doctor(&mut self) -> Result<(), DotsError> {
        self.log("Checking dependencies...".to_string())?;
        Ok(())
    }

    pub fn dependencies_install(&mut self) -> Result<(), DotsError> {
        self.log("Installing dependencies...".to_string())?;

        self.install("zsh", &PackageManager::ArchPacman)?;
        self.install("git", &PackageManager::ArchPacman)?;
        self.install("tealdeer", &PackageManager::RustCargo)?;

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

        Ok(())
    }

    pub fn dotfiles_uninstall(&mut self) -> Result<(), DotsError> {
        self.log("Uninstalling dotfiles...".to_string())?;
        Ok(())
    }
}
