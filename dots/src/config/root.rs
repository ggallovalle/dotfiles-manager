// root module for config

use std::path::PathBuf;
use indexmap::IndexMap;
use regex::Regex;
use lazy_static::lazy_static;
use dirs_next;
use std::env;

pub fn example_root() {
    println!("This is the root config module");
}

#[derive(Debug, Clone)]
pub struct RootConfig {
    pub dotfiles_dir: PathBuf,
    pub package_manager: Vec<String>,
    pub env: IndexMap<String, String>,
}

impl RootConfig {
    pub fn with_env_extended(&self, additional_env: &IndexMap<String, String>) -> Self {
        let mut new_env = self.env.clone();
        for (key, value) in additional_env {
            let expanded_value = expand_env_string(value, &new_env);
            new_env.insert(key.clone(), expanded_value);
        }
        RootConfig {
            dotfiles_dir: self.dotfiles_dir.clone(),
            package_manager: self.package_manager.clone(),
            env: new_env,
        }
    }

    pub fn env_expand(&self, value: &str) -> String {
        expand_env_string(value, &self.env)
    }

    pub fn env_home(&self) -> Option<PathBuf> {
        self.env.get("HOME").map(|h| PathBuf::from(h))
    }

    pub fn env_config_home(&self) -> Option<PathBuf> {
        self.env.get("XDG_CONFIG_HOME").map(|h| PathBuf::from(h))
    }

    pub fn env_data_home(&self) -> Option<PathBuf> {
        self.env.get("XDG_DATA_HOME").map(|h| PathBuf::from(h))
    }

    pub fn env_cache_home(&self) -> Option<PathBuf> {
        self.env.get("XDG_CACHE_HOME").map(|h| PathBuf::from(h))
    }

    pub fn env_shell(&self) -> Option<String> {
        self.env.get("SHELL").cloned()
    }

    pub fn env_editor(&self) -> Option<String> {
        self.env.get("EDITOR").cloned()
    }
}

impl Default for RootConfig {
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
        
        
        RootConfig {
            dotfiles_dir: dotfiles_dir,
            package_manager: vec![],
            env,
        }
    }
}



lazy_static! {
    // https://man.archlinux.org/man/environment.d.5
    static ref ENV_VAR_RE: Regex = Regex::new(r"\$(\w+)|\$\{(\w+)(?::([-+])([^}]*))?\}").unwrap();
}

fn expand_env_string(input: &str, env: &IndexMap<String, String>) -> String {
    ENV_VAR_RE.replace_all(input, |caps: &regex::Captures| {
        if let Some(var) = caps.get(1) {
            // Handle $VAR
            env.get(var.as_str()).cloned().unwrap_or_default()
        } else {
            let key = caps.get(2).unwrap().as_str();
            let op = caps.get(3).map(|m| m.as_str());
            let val = env.get(key).cloned().unwrap_or_default();

            match op {
                Some("-") => {
                    if val.is_empty() {
                        caps.get(4).unwrap().as_str().to_string()
                    } else {
                        val
                    }
                }
                Some("+") => {
                    if val.is_empty() {
                        "".to_string()
                    } else {
                        caps.get(4).unwrap().as_str().to_string()
                    }
                }
                _ => val,
            }
        }
    }).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env_string() {
        let mut env = IndexMap::new();
        env.insert("VAR1".to_string(), "value1".to_string());
        env.insert("VAR2".to_string(), "".to_string());
        env.insert("VAR3".to_string(), "value3".to_string());

        assert_eq!(expand_env_string("Path is $VAR1", &env), "Path is value1");
        assert_eq!(expand_env_string("Path is ${VAR1}", &env), "Path is value1");
        assert_eq!(expand_env_string("Path is ${VAR2:-default}", &env), "Path is default");
        assert_eq!(expand_env_string("Path is ${VAR3:+set}", &env), "Path is set");
        assert_eq!(expand_env_string("Path is ${VAR2:+set}", &env), "Path is ");
        assert_eq!(expand_env_string("No var here", &env), "No var here");
    }

    #[test]
    fn test_root_config_with_env_extended() {
        let base_config = RootConfig::default();
        let mut additional_env = IndexMap::new();
        additional_env.insert("NEW_VAR".to_string(), "new_value".to_string());
        additional_env.insert("HOME".to_string(), "/custom/home".to_string());
        additional_env.insert("XDG_CONFIG_HOME".to_string(), "$HOME/.config".to_string());
        additional_env.insert("ZDOTDIR".to_string(), "${XDG_CONFIG_HOME}/zsh".to_string());

        let new_config = base_config.with_env_extended(&additional_env);
        assert_eq!(new_config.env.get("NEW_VAR").unwrap(), "new_value");
        assert_eq!(new_config.env.get("HOME").unwrap(), "/custom/home");
        assert_eq!(new_config.env.get("XDG_CONFIG_HOME").unwrap(), "/custom/home/.config");
        assert_eq!(new_config.env.get("ZDOTDIR").unwrap(), "/custom/home/.config/zsh");
        let value = new_config.env_expand("$ZDOTDIR/home");
        assert_eq!(value, "/custom/home/.config/zsh/home");
    }

    #[test]
    fn test_root_config_default_extracts_from_env()
    {
        let root_config = RootConfig::default();
        let home = dirs_next::home_dir();
        let config_home = dirs_next::config_dir();
        let data_home = dirs_next::data_dir();
        let cache_home = dirs_next::cache_dir();

        assert_eq!(root_config.env_home(), home);
        assert_eq!(root_config.env_config_home(), config_home);
        assert_eq!(root_config.env_data_home(), data_home);
        assert_eq!(root_config.env_cache_home(), cache_home);
        assert_eq!(root_config.env_shell(), env::var("SHELL").ok());
        assert_eq!(root_config.env_editor(), env::var("EDITOR").ok());
    }
}
