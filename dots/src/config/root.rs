// root module for config

use crate::config::env::EnvironmentVariables;
use dirs_next;
use indexmap::IndexMap;
use std::env;
use std::path::PathBuf;

pub fn example_root() {
    println!("This is the root config module");
}

#[derive(Debug, Clone)]
pub struct Options {
    pub dotfiles_dir: PathBuf,
    pub package_manager: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: EnvironmentVariables,
    pub options: Options,
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
        let options = Options { dotfiles_dir, package_manager: vec![] };
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
}
