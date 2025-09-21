#![allow(unused)]

use miette;
use std::{fmt::Write, path::PathBuf};
use thiserror::Error;

mod config;

use crate::config::root::PackageManager;

#[derive(Error, Debug, miette::Diagnostic)]
pub enum DotsError {
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
    #[error("fmt error: {0}")]
    Fmt(#[from] std::fmt::Error),
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

        Ok(Dots { path, logs: String::new(), dry_run, bundles: the_bundles })
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
