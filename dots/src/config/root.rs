// root module for config

use crate::config::env::EnvironmentVariables;
use dirs_next;
use indexmap::IndexMap;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use which::which;

pub fn example_root() {
    println!("This is the root config module");
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: EnvironmentVariables,
    pub options: Options,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub dotfiles_dir: PathBuf,
    pub package_manager: IndexMap<PackageManager, PathBuf>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum PackageManager {
    ArchPacman,
    ArchYay,
    ArchParu,
    DebianApt,
    MacBrew,
    WindowsChoco,
    WindowsWinget,
    RustCargo,
}

impl PackageManager {
    pub fn which(&self) -> Option<PathBuf> {
        let command = self.to_string();
        which(command).ok()
    }
}

impl FromStr for PackageManager {
    type Err = Vec<&'static str>;

    fn from_str(input: &str) -> Result<PackageManager, Self::Err> {
        match input.to_lowercase().as_str() {
            "pacman" => Ok(PackageManager::ArchPacman),
            "yay" => Ok(PackageManager::ArchYay),
            "paru" => Ok(PackageManager::ArchParu),
            "apt" => Ok(PackageManager::DebianApt),
            "brew" => Ok(PackageManager::MacBrew),
            "choco" => Ok(PackageManager::WindowsChoco),
            "winget" => Ok(PackageManager::WindowsWinget),
            "cargo" => Ok(PackageManager::RustCargo),
            _ => Err(vec!["pacman", "yay", "paru", "apt", "brew", "choco", "winget", "cargo"]),
        }
    }
}

impl ToString for PackageManager {
    fn to_string(&self) -> String {
        match self {
            PackageManager::ArchPacman => "pacman",
            PackageManager::ArchYay => "yay",
            PackageManager::ArchParu => "paru",
            PackageManager::DebianApt => "apt",
            PackageManager::MacBrew => "brew",
            PackageManager::WindowsChoco => "choco",
            PackageManager::WindowsWinget => "winget",
            PackageManager::RustCargo => "cargo",
        }
        .to_string()
    }
}

impl Options {
    pub fn create(dotfiles_dir: PathBuf) -> Self {
        Options { dotfiles_dir, package_manager: Options::resolve_system_package_manager() }
    }

    fn resolve_system_package_manager() -> IndexMap<PackageManager, PathBuf> {
        let mut managers = IndexMap::new();
        macro_rules! pkg {
            ( $package:expr) => {
                if let Some(path) = $package.which() {
                    managers.insert($package, path);
                }
            };
        }
        #[cfg(target_os = "windows")]
        {
            pkg!(PackageManager::WindowsChoco);
            pkg!(PackageManager::WindowsWinget);
        }

        #[cfg(target_os = "linux")]
        {
            // archlinux
            pkg!(PackageManager::ArchYay);
            pkg!(PackageManager::ArchParu);
            pkg!(PackageManager::ArchPacman);
            // debian/ubuntu
            pkg!(PackageManager::DebianApt);
        }

        #[cfg(target_os = "macos")]
        {
            // macOS
            pkg!(PackageManager::MacBrew);
        }

        pkg!(PackageManager::RustCargo);

        managers
    }
}

impl Default for Config {
    fn default() -> Self {
        let mut env = IndexMap::new();
        let mut dotfiles_dir = PathBuf::new();
        if let Some(home) = dirs_next::home_dir() {
            env.insert("HOME".to_string(), home.to_string_lossy().to_string());
            dotfiles_dir = home.join("dotfiles");
        }
        if let Some(config) = dirs_next::config_dir() {
            env.insert("XDG_CONFIG_HOME".to_string(), config.to_string_lossy().to_string());
        }
        if let Some(data) = dirs_next::data_dir() {
            env.insert("XDG_DATA_HOME".to_string(), data.to_string_lossy().to_string());
        }
        if let Some(cache) = dirs_next::cache_dir() {
            env.insert("XDG_CACHE_HOME".to_string(), cache.to_string_lossy().to_string());
        }
        if let Ok(var) = env::var("SHELL") {
            env.insert("SHELL".to_string(), var);
        }
        if let Ok(var) = env::var("EDITOR") {
            env.insert("EDITOR".to_string(), var);
        }

        let env_vars = EnvironmentVariables { env };
        let options = Options::create(dotfiles_dir);
        Config { env: env_vars, options }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_extracts_from_env() {
        let config = Config::default();
        let home = dirs_next::home_dir();
        let config_home = dirs_next::config_dir();
        let data_home = dirs_next::data_dir();
        let cache_home = dirs_next::cache_dir();

        assert_eq!(config.env.home(), home);
        assert_eq!(config.env.config_home(), config_home);
        assert_eq!(config.env.data_home(), data_home);
        assert_eq!(config.env.cache_home(), cache_home);
        assert_eq!(config.env.shell(), env::var("SHELL").ok());
        assert_eq!(config.env.editor(), env::var("EDITOR").ok());
    }

    #[test]
    fn test_package_manager_string() {
        let pm = PackageManager::from_str("PACMAN").unwrap();
        assert_eq!(pm, PackageManager::ArchPacman);
        assert_eq!(pm.to_string(), "pacman".to_string());
        let pm = PackageManager::from_str("paru").unwrap();
        assert_eq!(pm, PackageManager::ArchParu);
        assert_eq!(pm.to_string(), "paru".to_string());
        let pm = PackageManager::from_str("yay").unwrap();
        assert_eq!(pm, PackageManager::ArchYay);
        assert_eq!(pm.to_string(), "yay".to_string());
        let pm = PackageManager::from_str("apt").unwrap();
        assert_eq!(pm, PackageManager::DebianApt);
        assert_eq!(pm.to_string(), "apt".to_string());
        let pm = PackageManager::from_str("brew").unwrap();
        assert_eq!(pm, PackageManager::MacBrew);
        assert_eq!(pm.to_string(), "brew".to_string());
        let pm = PackageManager::from_str("choco").unwrap();
        assert_eq!(pm, PackageManager::WindowsChoco);
        assert_eq!(pm.to_string(), "choco".to_string());
        let pm = PackageManager::from_str("winget").unwrap();
        assert_eq!(pm, PackageManager::WindowsWinget);
        assert_eq!(pm.to_string(), "winget".to_string());
        let pm = PackageManager::from_str("cargo").unwrap();
        assert_eq!(pm, PackageManager::RustCargo);
        assert_eq!(pm.to_string(), "cargo".to_string());
        let pm = PackageManager::from_str("unknown");
        assert!(pm.is_err());
    }
}
