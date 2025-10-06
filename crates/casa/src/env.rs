use indexmap::{IndexMap, IndexSet};
use std::{collections::HashSet, path::PathBuf, sync::Arc};

use subst;

use crate::config::KdlItemRef;

#[derive(Debug)]
pub enum EnvValue {
    /// plain string
    String(String),
    /// single path
    Path(PathBuf),
}

impl AsRef<str> for EnvValue {
    fn as_ref(&self) -> &str {
        match self {
            EnvValue::String(s) => s.as_ref(),
            EnvValue::Path(p) => p.to_str().unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct EnvItemMeta {
    pub inherited: bool,
    pub exported: bool,
    pub span: Option<KdlItemRef>,
}

#[derive(Debug, Clone)]
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

    pub fn apply_xdg(&mut self) -> &mut Self {
        if let Some(home) = dirs_next::home_dir() {
            self.insert(
                "HOME".to_owned(),
                EnvValue::Path(home),
                EnvItemMeta { inherited: true, exported: false, span: None },
            );
        }
        if let Some(user) = std::env::var("USER").ok() {
            self.insert(
                "USER".to_owned(),
                EnvValue::String(user),
                EnvItemMeta { inherited: true, exported: false, span: None },
            );
        }
        if let Some(config) = dirs_next::config_dir() {
            self.insert(
                "XDG_CONFIG_HOME".to_owned(),
                EnvValue::Path(config),
                EnvItemMeta { inherited: true, exported: false, span: None },
            );
        }
        if let Some(data) = dirs_next::data_dir() {
            self.insert(
                "XDG_DATA_HOME".to_owned(),
                EnvValue::Path(data),
                EnvItemMeta { inherited: true, exported: false, span: None },
            );
        }
        if let Some(cache) = dirs_next::cache_dir() {
            self.insert(
                "XDG_CACHE_HOME".to_owned(),
                EnvValue::Path(cache),
                EnvItemMeta { inherited: true, exported: false, span: None },
            );
        }
        self
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

    pub fn keys(&self) -> IndexSet<&String> {
        let mut keys = IndexSet::with_capacity(self.len());
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
