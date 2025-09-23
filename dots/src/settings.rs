use std::path::PathBuf;

use crate::{
    diag,
    settings::{
        env::ExpandValue,
        kdl_helpers::{FromKdlEntry, KdlDocumentExt},
    },
    settings_error::SettingsDiagnostic,
};
use indexmap::IndexMap;
use kdl::{KdlDiagnostic, KdlDocument, KdlEntry, KdlNode, KdlValue};
use miette::{Severity, SourceSpan};

#[derive(Debug, Clone)]
pub struct Settings {
    pub env: IndexMap<String, String>,
    env_inherited_keys: Vec<String>,
    pub dotfiles_dir: PathBuf,
    pub bundles: IndexMap<String, Vec<BundleItem>>,
}

#[derive(Debug, Clone)]
pub enum BundleItem {
    Install { name: String, span: SourceSpan },
    Copy { source: PathBuf, target: PathBuf, span: SourceSpan },
    Alias { from: String, to: String, span: SourceSpan },
    Clone { repo: String, target: PathBuf, span: SourceSpan },
}

impl Settings {
    pub fn from_kdl(document: KdlDocument) -> Result<Self, SettingsDiagnostic> {
        let mut env_map = env::base();
        let env_inherited_keys = env_map.keys().cloned().collect::<Vec<_>>();
        let dotfiles_dir = document
            .get_node_required_one("dotfiles_dir")
            .and_then(kdl_helpers::arg0)
            .and_then(|entry| env::ExpandValue::from_kdl_entry_dir_exists(entry, &env_map))?;

        env::ExpandValue::apply_exports_to_env(&document, &mut env_map)?;

        let mut bundles: IndexMap<String, Vec<BundleItem>> = IndexMap::new();

        for bundle in document.get_children_named("bundle") {
            let bundle_name = kdl_helpers::arg0(bundle).and_then(String::from_kdl_entry)?;
            if bundles.contains_key(&bundle_name) {
                return Err(diag!(
                    bundle.span(),
                    message = format!(
                        "node '{}' with id '{}' can only be specified once",
                        bundle.name().value(),
                        bundle_name
                    )
                )
                .into());
            }
            env::ExpandValue::apply_exports_to_env(bundle, &mut env_map)?;
            let mut items = Vec::new();
            bundles.insert(bundle_name, items);
        }

        Ok(Settings { env: env_map, env_inherited_keys, dotfiles_dir, bundles })
    }
}

impl env::ExpandValue {
    fn apply_exports_to_env(
        document: &impl kdl_helpers::KdlDocumentExt,
        env_map: &mut IndexMap<String, String>,
    ) -> Result<(), SettingsDiagnostic> {
        for n in document.get_children_named("export") {
            let (id, entry) = kdl_helpers::prop0(n)?;
            let v = env::ExpandValue::from_kdl_entry(entry, env_map)?;
            env_map.insert(id.value().to_string(), v.value);
        }
        Ok(())
    }

    fn from_kdl_entry(
        entry: &KdlEntry,
        env: &IndexMap<String, String>,
    ) -> Result<Self, KdlDiagnostic> {
        match env::expand(&String::from_kdl_entry(entry)?, env) {
            Err(e) => {
                // NOTE: maybe later, right now this logic doesn't account for comments or spacing
                // let name_len = match entry.name() {
                //     None => 0,
                //     Some(name_id) => name_id.span().len(),
                // };
                // let eq_and_left_quote_len = 2;
                // let property_value_offset =
                //     entry.span().offset() + name_len + eq_and_left_quote_len;
                // let offset = property_value_offset + e.offset;
                // let value_span: SourceSpan = (offset, e.len).into();
                return Err(diag!(
                    entry.span(),
                    message = format!("failed to expand env '{}'", e.var),
                    help = format!(
                        "available env vars: {}",
                        env.keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                    severity = Severity::Warning
                ));
            }
            Ok(expanded) => Ok(expanded),
        }
    }

    pub fn from_kdl_entry_dir_exists(
        entry: &KdlEntry,
        env: &IndexMap<String, String>,
    ) -> Result<PathBuf, KdlDiagnostic> {
        let expanded = Self::from_kdl_entry(entry, env)?;
        let path = PathBuf::from(&expanded.value);
        match path.metadata() {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(diag!(
                    entry.span(),
                    message = format!("path does not exist: {}", expanded.value),
                    help = "ensure the path exists, or update the configuration",
                    severity = Severity::Warning
                ));
            }
            Err(err) => {
                return Err(diag!(
                    entry.span(),
                    message = format!("failed to access path {}: {}", expanded.value, err),
                    help =
                        "check the path permissions or system state, or update the configuration",
                    severity = Severity::Warning
                ));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(diag!(
                    entry.span(),
                    message = format!("path is not a directory: {}", expanded.value),
                    help = "ensure the path is a directory, or update the configuration",
                    severity = Severity::Warning
                ));
            }
            Ok(_) => { /* path exists and is a directory */ }
        }
        Ok(path)
    }
}

mod kdl_helpers {
    use kdl::{KdlDiagnostic, KdlEntry};

    #[macro_export]
    macro_rules! diag {
    (
        $span:expr
        $(, input = $input:expr)?
        $(, label = $label:expr)?
        $(, message = $message:expr)?
        $(, help = $help:expr)?
        $(,)?
        , severity = $severity:expr
    ) => {

        KdlDiagnostic {
            span: $span,
            input: std::sync::Arc::new(String::new()) $(.or_else(|| Some($input.to_string())).unwrap_or_default().into())?,
            label: None $(.or(Some($label.to_string())))?,
            message: None $(.or(Some($message.to_string())))?,
            help: None $(.or(Some($help.to_string())))?,
            severity: $severity,
        }
    };
    (
        $span:expr
        $(, input = $input:expr)?
        $(, label = $label:expr)?
        $(, message = $message:expr)?
        $(, help = $help:expr)?
        $(,)?
    ) => {

        KdlDiagnostic {
            span: $span,
            input: std::sync::Arc::new(String::new()) $(.or_else(|| Some($input.to_string())).unwrap_or_default().into())?,
            label: None $(.or(Some($label.to_string())))?,
            message: None $(.or(Some($message.to_string())))?,
            help: None $(.or(Some($help.to_string())))?,
            severity: miette::Severity::default(),
        }
    };

    }

    macro_rules! bail_on_entry_ty {
        ( $entry:expr ) => {
            match $entry.ty() {
                None => {}
                Some(identifier) => {
                    return Err(diag!(
                        identifier.span(),
                        message = format!(
                            "type annotations are not supported on this entry, found: {}",
                            identifier.value()
                        )
                    ))
                }
            }
        };
    }

    pub fn inspect_entry_ty_name(entry: &KdlEntry) -> &str {
        match entry.ty() {
            None => match entry.value() {
                kdl::KdlValue::String(_) => "string",
                kdl::KdlValue::Integer(_) => "int",
                kdl::KdlValue::Float(_) => "float",
                kdl::KdlValue::Bool(_) => "bool",
                kdl::KdlValue::Null => "null",
            },
            Some(identifier) => identifier.value(),
        }
    }

    pub fn prop0(node: &kdl::KdlNode) -> Result<(&kdl::KdlIdentifier, &KdlEntry), KdlDiagnostic> {
        let mut entries = node.entries().iter();
        let name = node.name().value();
        match entries.next() {
            None => Err(diag!(
                node.span(),
                message = format!("node '{}' requires at least one entry", name)
            )),
            Some(entry) => match entry.name() {
                None => Err(diag!(
                    entry.span(),
                    message =
                        format!("node '{}' first entry must be a property, not an argument", name)
                )),
                Some(name_id) => Ok((name_id, entry)),
            },
        }
    }

    pub fn arg0(node: &kdl::KdlNode) -> Result<&KdlEntry, KdlDiagnostic> {
        let mut entries = node.entries().iter();
        match entries.next() {
            None => Err(diag!(
                node.span(),
                message = format!("node '{}' requires at least one entry", node.name().value())
            )),
            Some(entry) => match entry.name() {
                Some(name_id) => Err(diag!(
                    name_id.span(),
                    message = format!(
                        "node '{}' first entry must be an argument, not a property",
                        node.name().value()
                    )
                )),
                None => Ok(entry),
            },
        }
    }

    pub trait FromKdlEntry: Sized {
        fn from_kdl_entry(entry: &kdl::KdlEntry) -> Result<Self, KdlDiagnostic>;
    }

    impl FromKdlEntry for String {
        fn from_kdl_entry(entry: &kdl::KdlEntry) -> Result<Self, KdlDiagnostic> {
            bail_on_entry_ty!(entry);
            entry.value().as_string().map(|s| s.to_owned()).ok_or_else(|| {
                diag!(
                    entry.span(),
                    message = format!(
                        "invalid type: {}, expected: {}",
                        inspect_entry_ty_name(entry),
                        "string"
                    )
                )
            })
        }
    }

    pub trait KdlDocumentExt {
        fn get_span(&self) -> miette::SourceSpan;
        fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode>;
        fn get_children_named<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a kdl::KdlNode> {
            self.get_children().filter(move |n| n.name().value() == name)
        }
        fn get_node_required_one(&self, name: &str) -> Result<&kdl::KdlNode, KdlDiagnostic> {
            let mut found = None;
            for node in self.get_children() {
                if node.name().value() == name {
                    if found.is_some() {
                        return Err(diag!(
                            node.span(),
                            message = format!("node '{}' can only be specified once", name)
                        ));
                    }
                    found = Some(node);
                }
            }
            if found.is_none() {
                return Err(diag!(
                    self.get_span(),
                    message = format!("node '{}' is required", name)
                ));
            }
            Ok(found.unwrap())
        }
    }

    impl KdlDocumentExt for kdl::KdlDocument {
        fn get_span(&self) -> miette::SourceSpan {
            self.span()
        }

        fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode> {
            self.nodes().iter()
        }
    }

    impl KdlDocumentExt for kdl::KdlNode {
        fn get_span(&self) -> miette::SourceSpan {
            self.span()
        }

        fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode> {
            match self.children() {
                None => either::Left(core::iter::empty()),
                Some(doc) => either::Right(doc.get_children()),
            }
        }
    }
}

mod env {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExpandError {
        pub var: String,
        pub offset: usize,
        pub len: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ExpandValue {
        pub value: String,
        pub raw: String,
        pub replacement_count: usize,
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
    use indexmap::IndexMap;
    use lazy_static::lazy_static;
    use regex::Regex;
    use std::path::PathBuf;

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
}

#[cfg(test)]
mod tests {
    use super::env::*;
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
