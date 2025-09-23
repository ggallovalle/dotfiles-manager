use std::path::PathBuf;

use indexmap::IndexMap;
use strum::{self, VariantNames};
use which::which;

#[derive(
    Debug, Clone, Hash, PartialEq, Eq, strum::EnumString, strum::VariantNames, strum::Display,
)]
pub enum PackageManager {
    #[strum(serialize = "pacman")]
    ArchPacman,
    #[strum(serialize = "yay")]
    ArchYay,
    #[strum(serialize = "paru")]
    ArchParu,
    #[strum(serialize = "apt")]
    DebianApt,
    #[strum(serialize = "brew")]
    MacBrew,
    #[strum(serialize = "choco")]
    WindowsChoco,
    #[strum(serialize = "winget")]
    WindowsWinget,
    #[strum(serialize = "cargo")]
    RustCargo,
}

impl PackageManager {
    pub fn which(&self) -> Option<PathBuf> {
        let command = self.to_string();
        which(command).ok()
    }
}
