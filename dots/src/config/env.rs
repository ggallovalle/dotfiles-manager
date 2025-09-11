use indexmap::IndexMap;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct EnvironmentVariables {
    pub env: IndexMap<String, String>,
}

impl EnvironmentVariables {
    pub fn with_env_extended(&self, additional_env: &IndexMap<String, String>) -> Self {
        let mut new_env = self.env.clone();
        for (key, value) in additional_env {
            let expanded_value = expand_env_string(value, &new_env);
            new_env.insert(key.clone(), expanded_value);
        }
        EnvironmentVariables { env: new_env }
    }

    pub fn expand(&self, value: &str) -> String {
        expand_env_string(value, &self.env)
    }

    pub fn home(&self) -> Option<PathBuf> {
        self.env.get("HOME").map(|h| PathBuf::from(h))
    }

    pub fn config_home(&self) -> Option<PathBuf> {
        self.env.get("XDG_CONFIG_HOME").map(|h| PathBuf::from(h))
    }

    pub fn data_home(&self) -> Option<PathBuf> {
        self.env.get("XDG_DATA_HOME").map(|h| PathBuf::from(h))
    }

    pub fn cache_home(&self) -> Option<PathBuf> {
        self.env.get("XDG_CACHE_HOME").map(|h| PathBuf::from(h))
    }

    pub fn shell(&self) -> Option<String> {
        self.env.get("SHELL").cloned()
    }

    pub fn editor(&self) -> Option<String> {
        self.env.get("EDITOR").cloned()
    }
}

lazy_static! {
    // https://man.archlinux.org/man/environment.d.5
    static ref ENV_VAR_RE: Regex = Regex::new(r"\$(\w+)|\$\{(\w+)(?::([-+])([^}]*))?\}").unwrap();
}

fn expand_env_string(input: &str, env: &IndexMap<String, String>) -> String {
    ENV_VAR_RE
        .replace_all(input, |caps: &regex::Captures| {
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
        })
        .into_owned()
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
    fn test_env_vars_with_env_extended() {
        let base_env = EnvironmentVariables { env: IndexMap::new() };
        let mut additional_env = IndexMap::new();
        additional_env.insert("NEW_VAR".to_string(), "new_value".to_string());
        additional_env.insert("HOME".to_string(), "/custom/home".to_string());
        additional_env.insert("XDG_CONFIG_HOME".to_string(), "$HOME/.config".to_string());
        additional_env.insert("ZDOTDIR".to_string(), "${XDG_CONFIG_HOME}/zsh".to_string());

        let new_env = base_env.with_env_extended(&additional_env);
        assert_eq!(new_env.env.get("NEW_VAR").unwrap(), "new_value");
        assert_eq!(new_env.env.get("HOME").unwrap(), "/custom/home");
        assert_eq!(new_env.env.get("XDG_CONFIG_HOME").unwrap(), "/custom/home/.config");
        assert_eq!(new_env.env.get("ZDOTDIR").unwrap(), "/custom/home/.config/zsh");
        let value = new_env.expand("$ZDOTDIR/home");
        assert_eq!(value, "/custom/home/.config/zsh/home");
    }
}
