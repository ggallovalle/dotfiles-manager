use indexmap::IndexMap;
use lazy_static::lazy_static;
use regex::Regex;
use std::{collections::HashSet, path::PathBuf, sync::Arc};

use subst;

use crate::config::KdlItemRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandValue {
    pub value: String,
    pub raw: String,
    pub replacement_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandError {
    pub var: String,
    pub offset: usize,
    pub len: usize,
}

impl std::fmt::Display for ExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "environment variable '${}' not found at offset {} (len {})",
            self.var, self.offset, self.len
        )
    }
}

impl std::error::Error for ExpandError {}

lazy_static! {
    // https://man.archlinux.org/man/environment.d.5
    static ref ENV_VAR_RE: Regex = Regex::new(r"\$(\w+)|\$\{(\w+)(?::([-+])([^}]*))?\}").unwrap();
}

/// Expands environment variables in the input string using the provided environment map.
/// Supports the following forms:
/// - `$VAR`
/// - `${VAR}`
/// - `${VAR:-default}` (uses `default` if `VAR` is unset or empty)
/// - `${VAR:+alt}` (uses `alt` if `VAR` is set and non-empty)
/// Returns an `ExpandValue` on success or an `ExpandError` if a variable is not found in the env map.
pub fn expand(input: &str, env: &IndexMap<String, String>) -> Result<ExpandValue, ExpandError> {
    let mut result = String::with_capacity(input.len());
    let mut last = 0;
    let mut replacement_count = 0;
    for caps in ENV_VAR_RE.captures_iter(input) {
        let m = caps.get(0).unwrap();
        // push text before the match
        result.push_str(&input[last..m.start()]);
        if let Some(var) = caps.get(1) {
            // $VAR
            let var_name = var.as_str();
            if let Some(env_val) = env.get(var_name) {
                replacement_count += 1;
                result.push_str(env_val);
            } else {
                return Err(ExpandError {
                    var: var_name.to_string(),
                    offset: m.start(),
                    len: m.end() - m.start(),
                });
            }
        } else {
            let key = caps.get(2).unwrap().as_str();
            let op = caps.get(3).map(|m| m.as_str());
            let env_val = env.get(key).cloned().unwrap_or_default();
            match op {
                Some("-") => {
                    // ${VAR:-default}
                    replacement_count += 1;
                    if env_val.is_empty() {
                        result.push_str(caps.get(4).unwrap().as_str());
                    } else {
                        result.push_str(&env_val);
                    }
                }
                Some("+") => {
                    // ${VAR:+alt}
                    if env_val.is_empty() {
                        // nothing
                    } else {
                        result.push_str(caps.get(4).unwrap().as_str());
                    }
                }
                _ => {
                    // ${VAR}
                    if env_val.is_empty() {
                        return Err(ExpandError {
                            var: key.to_string(),
                            offset: m.start(),
                            len: m.end() - m.start(),
                        });
                    } else {
                        replacement_count += 1;
                        result.push_str(&env_val);
                    }
                }
            }
        }
        last = m.end();
    }
    result.push_str(&input[last..]);
    Ok(ExpandValue { value: result, raw: input.to_string(), replacement_count })
}

pub fn base() -> IndexMap<String, String> {
    let mut env = IndexMap::new();
    if let Some(user) = std::env::var("USER").ok() {
        env.insert("USER".to_string(), user);
    }
    if let Some(home) = dirs_next::home_dir() {
        env.insert("HOME".to_string(), home.to_string_lossy().to_string());
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
    env
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

        // doesn't use env variables
        let expanded = expand("No var here", &env).unwrap();
        assert_eq!(expanded.value, "No var here");
        assert_eq!(expanded.replacement_count, 0);

        // expands variables
        let expanded = expand("Path is $VAR1", &env).unwrap();
        assert_eq!(expanded.value, "Path is value1");
        let expanded = expand("Path is ${VAR1}", &env).unwrap();
        assert_eq!(expanded.value, "Path is value1");
        // default
        assert_eq!(expand("Path is ${VAR2:-default}", &env).unwrap().value, "Path is default");
        // alt
        assert_eq!(expand("Path is ${VAR3:+set}", &env).unwrap().value, "Path is set");
        assert_eq!(expand("Path is ${VAR2:+set}", &env).unwrap().value, "Path is ");

        // multiple
        let expanded = expand("Values: $VAR1, ${VAR3}, ${VAR2:-def}", &env).unwrap();
        assert_eq!(expanded.value, "Values: value1, value3, def");
        assert_eq!(expanded.replacement_count, 3);

        // Test errors
        let err = expand("Missing $XDG_CONFIG_HOME", &env).unwrap_err();
        assert_eq!(err, ExpandError { var: "XDG_CONFIG_HOME".to_string(), offset: 8, len: 16 });
        let err = expand("Not Found ${HOME}", &env).unwrap_err();
        assert_eq!(err, ExpandError { var: "HOME".to_string(), offset: 10, len: 7 });
    }
}

#[derive(Debug)]
pub enum EnvValue {
    /// plain string
    String(String),
    /// single path
    Path(String),
}

impl AsRef<str> for EnvValue {
    fn as_ref(&self) -> &str {
        match self {
            EnvValue::String(s) => s.as_ref(),
            EnvValue::Path(p) => p.as_ref(),
        }
    }
}

#[derive(Debug)]
pub struct EnvItemMeta {
    inherited: bool,
    exported: bool,
    span: Option<KdlItemRef>,
}

#[derive(Debug)]
pub struct Env {
    inner: Arc<IndexMap<String, Arc<(EnvValue, EnvItemMeta)>>>,
    parent: Option<Arc<IndexMap<String, Arc<(EnvValue, EnvItemMeta)>>>>,
}

impl Env {
    pub fn empty() -> Self {
        Env { inner: Arc::new(IndexMap::new()), parent: None }
    }

    pub fn child(&self) -> Self {
        Env { inner: Arc::new(IndexMap::new()), parent: Some(self.inner.clone()) }
    }

    pub fn get<T: AsRef<str>>(&self, key: T) -> Option<&(EnvValue, EnvItemMeta)> {
        if let Some(v) = self.inner.get(key.as_ref()) {
            Some(v)
        } else if let Some(parent) = &self.parent {
            parent.get(key.as_ref()).map(|v| &**v)
        } else {
            None
        }
    }

    pub fn get_str<T: AsRef<str>>(&self, key: T) -> Option<&str> {
        self.get(key).map(|(v, _)| v.as_ref())
    }

    pub fn len(&self) -> usize {
        let parent_len = self.parent.as_ref().map(|p| p.len()).unwrap_or(0);
        parent_len + self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn insert(&mut self, key: String, value: EnvValue, meta: EnvItemMeta) {
        Arc::make_mut(&mut self.inner).insert(key, Arc::new((value, meta)));
    }

    pub fn insert_simple(&mut self, key: &str, value: &str) {
        let meta = EnvItemMeta { inherited: false, exported: false, span: None };
        Arc::make_mut(&mut self.inner)
            .insert(key.to_owned(), Arc::new((EnvValue::String(value.to_owned()), meta)));
    }

    pub fn keys(&self) -> HashSet<&String> {
        let mut keys = HashSet::with_capacity(self.len());
        for key in self.inner.keys() {
            keys.insert(key);
        }
        if let Some(parent) = &self.parent {
            for key in parent.keys() {
                keys.insert(key);
            }
        }
        keys
    }

    pub fn expand<T: AsRef<str>>(&self, source: T) -> Result<String, subst::Error> {
        subst::substitute(source.as_ref(), self)
    }
}

impl<'a> subst::VariableMap<'a> for Env {
    type Value = &'a str;

    fn get(&'a self, key: &str) -> Option<Self::Value> {
        self.get_str(key)
    }
}
