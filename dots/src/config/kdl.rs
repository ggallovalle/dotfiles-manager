use crate::config::bundle::{self, Bundle};
use crate::config::env::EnvironmentVariables;
use crate::config::root::Options;
use crate::config::root::PackageManager;
use either;
use semver;
use std::collections::HashMap;
use std::{fmt::Display, path::PathBuf};

use miette::{self, Diagnostic, LabeledSpan, NamedSource, SourceCode};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic, Clone)]
pub enum ConfigError {
    #[error("Deserialization error: {0}")]
    KdlDeserializationError(#[from] kdl::KdlError),
    #[error("kdl deserialization error: {0}")]
    #[diagnostic(transparent)]
    KdlError(KdlError), // TODO: consolidate these
}

impl ConfigError {
    pub fn new_kdl_error(error_message: String, offset: usize, len: usize) -> Self {
        ConfigError::KdlError(KdlError {
            error_message,
            src: None,
            offset: Some(offset),
            len: Some(len),
            help_message: None,
        })
    }

    pub fn set_help_message(mut self, help_message: String) -> Self {
        match &mut self {
            ConfigError::KdlError(kdl_error) => {
                kdl_error.help_message = Some(help_message);
            }
            _ => {}
        }
        self
    }
}

#[derive(Error, Debug, Clone)]
pub struct KdlError {
    pub error_message: String,
    pub src: Option<NamedSource<String>>,
    pub offset: Option<usize>,
    pub len: Option<usize>,
    pub help_message: Option<String>,
}

impl Display for KdlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "failed to parse Dots configuration")
    }
}

impl Diagnostic for KdlError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self.src.as_ref() {
            Some(src) => Some(src),
            None => None,
        }
    }

    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new("dots::config_kdl_error"))
    }

    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new(
            "https://github.com/ggallovalle/dotfiles-manager/blob/main/documentation/configuration.md",
        ))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match &self.help_message {
            Some(help_message) => Some(Box::new(help_message)),
            None => None,
        }
    }
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        if let (Some(offset), Some(len)) = (self.offset, self.len) {
            let label = LabeledSpan::new(Some(self.error_message.clone()), offset, len);
            Some(Box::new(std::iter::once(label)))
        } else {
            None
        }
    }
}

impl KdlError {
    pub fn add_src(mut self, src_name: String, src_input: String) -> Self {
        self.src = Some(NamedSource::new(src_name, src_input));
        self
    }
}

macro_rules! kdl_error {
    ( $message:expr, $entry:expr ) => {
        ConfigError::new_kdl_error($message, $entry.span().offset(), $entry.span().len())
    };
    ( $message:expr, $entry:expr, $help: expr ) => {
        kdl_error!($message, $entry).set_help_message($help)
    };
}

macro_rules! kdl_error_span {
    ( $message:expr, $span:expr ) => {
        ConfigError::new_kdl_error($message, $span.offset(), $span.len())
    };
    ( $message:expr, $span:expr, $help: expr ) => {
        kdl_error_span!($message, $span).set_help_message($help)
    };
}

macro_rules! kdl_error_required {
    ( $kdl_node:expr, $property_name:expr ) => {
        kdl_error!(format!("node {} is required", $property_name), $kdl_node)
    };
}

macro_rules! kdl_error_empty {
    ( $kdl_node:expr ) => {
        kdl_error!(
            "node can not be empty (without arguments)".to_string(),
            $kdl_node,
            "add at least one argument to the node".to_string()
        )
    };
}

macro_rules! kdl_error_invalid_type {
    ( $kdl_entry:expr, $type:expr ) => {
        kdl_error!(
            format!(
                "invalid type: {}, expected: {}",
                match $kdl_entry.ty() {
                    None => match $kdl_entry.value() {
                        kdl::KdlValue::String(_) => "string",
                        kdl::KdlValue::Integer(_) => "int",
                        kdl::KdlValue::Float(_) => "float",
                        kdl::KdlValue::Bool(_) => "bool",
                        kdl::KdlValue::Null => "null",
                    },
                    Some(identifier) => identifier.value(),
                },
                $type
            ),
            $kdl_entry
        )
    };
}

trait KdlEntryExtract: Sized {
    const TYPE_NAME: &'static str;
    fn extract(entry: &kdl::KdlEntry) -> Result<Self, ConfigError>;
}

impl KdlEntryExtract for String {
    const TYPE_NAME: &'static str = "string";
    fn extract(entry: &kdl::KdlEntry) -> Result<Self, ConfigError> {
        if entry.ty().is_some() {
            return Err(kdl_error_invalid_type!(entry, Self::TYPE_NAME));
        }
        entry
            .value()
            .as_string()
            .map(|s| s.to_owned())
            .ok_or(kdl_error_invalid_type!(entry, Self::TYPE_NAME))
    }
}

impl KdlEntryExtract for bool {
    const TYPE_NAME: &'static str = "bool";
    fn extract(entry: &kdl::KdlEntry) -> Result<Self, ConfigError> {
        if entry.ty().is_some() {
            return Err(kdl_error_invalid_type!(entry, Self::TYPE_NAME));
        }
        entry.value().as_bool().ok_or(kdl_error_invalid_type!(entry, Self::TYPE_NAME))
    }
}

impl KdlEntryExtract for PackageManager {
    const TYPE_NAME: &'static str = "package manager";
    fn extract(entry: &kdl::KdlEntry) -> Result<Self, ConfigError> {
        String::extract(entry).and_then(|m| {
            m.parse::<Self>().map_err(|valid_names| {
                kdl_error_span!(
                    format!("unknown varian: {}", m,),
                    entry.span(),
                    format!("select one of: {}", valid_names.join(", "))
                )
            })
        })
    }
}

trait KdlNodeExtract: Sized {
    const TYPE_NAME: &'static str;
    type Success;
    fn extract(node: &kdl::KdlNode) -> Result<Self::Success, ConfigError>;
}

impl<T> KdlNodeExtract for (String, T)
where
    T: KdlEntryExtract,
{
    type Success = ((String, T), miette::SourceSpan);
    const TYPE_NAME: &'static str = "key-value pair";
    fn extract(node: &kdl::KdlNode) -> Result<Self::Success, ConfigError> {
        let mut entries = node.entries().iter();
        let (key, value_entry) = match entries.next() {
            None => {
                return Err(kdl_error!("expected a key-value pair".to_string(), node));
            }
            Some(key_entry) => match key_entry.name() {
                Some(name) => (name.value(), key_entry),
                None => match entries.next() {
                    None => {
                        return Err(kdl_error!("expected a key-value pair".to_string(), node));
                    }
                    Some(value_entry)
                        if key_entry.value().is_string() && key_entry.ty().is_none() =>
                    {
                        let key = key_entry.value().as_string().unwrap();
                        (key, value_entry)
                    }
                    _ => {
                        return Err(kdl_error_invalid_type!(key_entry, "string"));
                    }
                },
            },
        };

        Ok(((key.to_string(), T::extract(value_entry)?), value_entry.span()))
    }
}

impl<T> KdlNodeExtract for Vec<T>
where
    T: KdlEntryExtract,
{
    type Success = Vec<(T, miette::SourceSpan)>;
    const TYPE_NAME: &'static str = "list";
    fn extract(node: &kdl::KdlNode) -> Result<Self::Success, ConfigError> {
        let mut values = vec![];
        for entry in node.entries().iter().filter(|e| e.name().is_none()) {
            values.push((T::extract(entry)?, entry.span()));
        }
        if values.is_empty() { Err(kdl_error_empty!(node)) } else { Ok(values) }
    }
}

trait KdlNodeExt {
    fn get_entry_as<T: KdlEntryExtract>(
        &self,
        key: impl Into<kdl::NodeKey>,
    ) -> Result<T, ConfigError>;
    fn get_entry_arg_as<T: KdlEntryExtract>(&self, index: usize) -> Result<T, ConfigError> {
        self.get_entry_as(index)
    }
    fn get_entry_prop_as<T: KdlEntryExtract>(&self, key: &str) -> Result<T, ConfigError> {
        self.get_entry_as(key)
    }

    fn try_get_entry_as<T: KdlEntryExtract>(
        &self,
        key: impl Into<kdl::NodeKey>,
    ) -> Result<Option<(T, &kdl::KdlEntry)>, ConfigError>;

    fn try_get_entry_arg_as<T: KdlEntryExtract>(
        &self,
        index: usize,
    ) -> Result<Option<(T, &kdl::KdlEntry)>, ConfigError> {
        self.try_get_entry_as(index)
    }

    fn try_get_entry_prop_as<T: KdlEntryExtract>(
        &self,
        key: &str,
    ) -> Result<Option<(T, &kdl::KdlEntry)>, ConfigError> {
        self.try_get_entry_as(key)
    }

    fn get_node_as<T: KdlNodeExtract>(&self) -> Result<T::Success, ConfigError>;
}

impl KdlNodeExt for kdl::KdlNode {
    fn get_entry_as<T: KdlEntryExtract>(
        &self,
        key: impl Into<kdl::NodeKey>,
    ) -> Result<T, ConfigError> {
        let key_node: kdl::NodeKey = key.into();
        let key_type = match &key_node {
            kdl::NodeKey::Key(name) => format!("missing property '{}'", name.value()),
            kdl::NodeKey::Index(index) => format!("missing argument #{}", index),
        };
        let entry = match self.entry(key_node) {
            None => {
                return Err(kdl_error!(key_type, self));
            }
            Some(e) => e,
        };

        T::extract(entry)
    }

    fn try_get_entry_as<T: KdlEntryExtract>(
        &self,
        key: impl Into<kdl::NodeKey>,
    ) -> Result<Option<(T, &kdl::KdlEntry)>, ConfigError> {
        let key_node: kdl::NodeKey = key.into();
        match self.entry(key_node) {
            None => Ok(None),
            Some(entry) => T::extract(entry).map(|val| Some((val, entry))),
        }
    }

    fn get_node_as<T: KdlNodeExtract>(&self) -> Result<T::Success, ConfigError> {
        T::extract(self)
    }
}

pub trait KdlNodeLookup {
    fn get_node(&self, name: &str) -> Option<&kdl::KdlNode>;
    fn get_node_required(&self, name: &str) -> Result<&kdl::KdlNode, ConfigError>;
    fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode>;
    fn get_children_named<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a kdl::KdlNode> {
        self.get_children().filter(move |n| n.name().value() == name)
    }
}

impl KdlNodeLookup for kdl::KdlDocument {
    fn get_node(&self, name: &str) -> Option<&kdl::KdlNode> {
        self.get(name)
    }
    fn get_node_required(&self, name: &str) -> Result<&kdl::KdlNode, ConfigError> {
        match self.get(name) {
            None => Err(kdl_error_required!(self, name)),
            Some(node) => Ok(node),
        }
    }
    fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode> {
        self.nodes().iter()
    }
}

impl KdlNodeLookup for kdl::KdlNode {
    fn get_node(&self, name: &str) -> Option<&kdl::KdlNode> {
        self.children().and_then(|doc| doc.get(name))
    }

    fn get_node_required(&self, name: &str) -> Result<&kdl::KdlNode, ConfigError> {
        match self.get_node(name) {
            None => Err(kdl_error_required!(self, name)),
            Some(node) => Ok(node),
        }
    }
    fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode> {
        match self.children() {
            None => either::Left(core::iter::empty()),
            Some(doc) => either::Right(doc.get_children()),
        }
    }
}

impl Options {
    pub fn from_kdl(root: &impl KdlNodeLookup) -> Result<Self, ConfigError> {
        let dotfiles_dir = root
            .get_node_required("dotfiles_dir")?
            .get_entry_arg_as::<String>(0)
            .map(PathBuf::from)?;

        let mut options = Options::create(dotfiles_dir);
        let managers = root
            .get_node_required("package_managers")
            .and_then(|node| node.get_node_as::<Vec<PackageManager>>())
            .and_then(|managers| {
                let available_in_system =
                    options.package_manager.keys().cloned().collect::<Vec<_>>();
                for (m, span) in &managers {
                    if !options.package_manager.contains_key(m) {
                        return Err(kdl_error_span!(
                            format!("{} is not available on this system", m.to_string()),
                            span,
                            format!(
                                "detected in this system are: {}",
                                available_in_system
                                    .iter()
                                    .map(|pm| pm.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        ));
                    }
                }
                Ok(managers)
            })?;

        // at this point I'm guaranteed to have at least one manager, because otherwise the
        // kdl error would have been raised above
        options.package_manager.retain(|pm, _| managers.iter().any(|(m, _s)| m == pm));

        Ok(options)
    }
}

impl EnvironmentVariables {
    pub fn apply_kdl(&mut self, root: &impl KdlNodeLookup) -> Result<(), ConfigError> {
        for env_node in root.get_children_named("env") {
            if let Ok(inherit) = env_node.get_entry_prop_as::<bool>("inherit")
                && let Ok(key) = env_node.get_entry_arg_as::<String>(0)
            {
                if inherit {
                    if let Ok(system_value) = std::env::var(&key) {
                        self.env.insert(key.to_string(), system_value);
                    }
                } else {
                    self.env.shift_remove(&key.to_string());
                }
                continue;
            }

            let ((key, value), _s) = env_node.get_node_as::<(String, String)>()?;
            self.env.insert(key.to_string(), self.expand(&value));
        }
        Ok(())
    }
}

impl Bundle {
    fn dependencies_from_kdl(
        deps_node: &impl KdlNodeLookup,
    ) -> Result<Vec<bundle::Dependency>, ConfigError> {
        let mut dependencies_hash = HashMap::new();
        for dep_item_node in deps_node.get_children() {
            let dep_name = dep_item_node.name().value();

            let dep_version = match dep_item_node.try_get_entry_arg_as::<String>(0)? {
                Some((v, e)) => Some((v, e)),
                None => match dep_item_node.try_get_entry_prop_as::<String>("version")? {
                    Some((v, e)) => Some((v, e)),
                    None => None,
                },
            };
            let dep_version_req = match &dep_version {
                Some((v, e)) => match semver::VersionReq::parse(v) {
                    Ok(req) => Some(req),
                    Err(err) => {
                        return Err(kdl_error!(
                                    format!("invalid version requirement: {}", err),
                                    e,
                                    "use a valid semver version requirement, see https://docs.rs/semver/latest/semver/struct.VersionReq.html#syntax".to_string()
                                ));
                    }
                },
                None => None,
            };
            if dependencies_hash.contains_key(dep_name) {
                return Err(kdl_error!(
                    format!("dependency {} is already defined", dep_name),
                    dep_item_node,
                    "remove the duplicate definition".to_string()
                ));
            }

            let manager = match dep_item_node.try_get_entry_prop_as::<PackageManager>("manager")? {
                Some((m, _s)) => Some(m),
                None => None,
            };

            dependencies_hash.insert(
                dep_name.to_string(),
                bundle::Dependency {
                    name: dep_name.to_string(),
                    version: dep_version_req,
                    manager: manager,
                },
            );
        }
        Ok(dependencies_hash.into_values().collect())
    }

    pub fn from_kdl(root: &impl KdlNodeLookup) -> Result<Vec<Self>, ConfigError> {
        let mut bundles = vec![];

        for bundle_node in root.get_children_named("bundle") {
            let name = bundle_node.get_entry_arg_as::<String>(0)?;
            let dependencies = match bundle_node.get_node("dependencies") {
                None => vec![],
                Some(deps_node) => Bundle::dependencies_from_kdl(deps_node)?,
            };
            bundles.push(Bundle { name, dependencies, dotfiles: vec![] });
        }

        Ok(bundles)
    }
}

#[cfg(test)]
mod tests {
    use kdl::KdlDocument;

    use super::*;

    #[test]
    fn test_options_from_kdl() {
        let kdl_str = r#"
            dotfiles_dir "/home/user/dotfiles"
            package_managers cargo // comment to fail with "property required" error

            // package_managers key="cargo" // fail with "node canot be empty" error

            // package_managers "unknown_pm" // fail with "invalid package manager" error

            // package_managers "winget" // fail with "not available on this system" error

            // package_managers #true // fail with "must be a string" error
            // package_managers // fail with "canot be empty" error
        "#;
        let kdl_doc: KdlDocument = kdl_str.parse().unwrap();
        let options = Options::from_kdl(&kdl_doc)
            .map_err(|err| miette::Error::new(err).with_source_code(kdl_str))
            .unwrap();
        assert_eq!(options.dotfiles_dir, PathBuf::from("/home/user/dotfiles"));
        assert_eq!(options.package_manager.len(), 1);
        assert!(options.package_manager.contains_key(&PackageManager::RustCargo));
    }

    #[test]
    fn test_env_apply_kdl() {
        unsafe {
            std::env::set_var("GIBRISH", "gibrish");
            std::env::set_var("GIBRISH_1", "gibrish-1");
        }
        let kdl_str = r#"
            // env key1 // error: expected key-value pair
            // env KEY1 12345 // error: invalid type on value
            // env 12345 VALUE1 // error: invalid type on key
            // env key1=(time)"value1" // error: invalid type on value
            // env HOME inherit=12345 // error: invalid type on inherit

            env GIBRISH inherit=#true
            env GIBRISH_1 inherit=#false
            env PLAIN inherit=#false
            env KEY1 "value1"
            env KEY2 "${KEY1}_suffix"
            env KEY3="${KEY2}_more"
            env KEY4="${GIBRISH}_extended"
            env KEY5 "${GIBRISH_1:-not-inherited}_extended"
            env KEY6 "${PLAIN:-removed}_extended"

            hello "ignored"
        "#;
        let kdl_doc: KdlDocument = kdl_str.parse().unwrap();
        let mut env = EnvironmentVariables::default();
        env.env.insert("PLAIN".to_string(), "im-plain".to_string());
        env.apply_kdl(&kdl_doc)
            .map_err(|err| miette::Error::new(err).with_source_code(kdl_str))
            .unwrap();
        assert_eq!(env.env.get("KEY1").unwrap(), "value1");
        assert_eq!(env.env.get("KEY2").unwrap(), "value1_suffix");
        assert_eq!(env.env.get("KEY3").unwrap(), "value1_suffix_more");
        assert_eq!(env.env.get("KEY4").unwrap(), "gibrish_extended");
        assert_eq!(env.env.get("KEY5").unwrap(), "not-inherited_extended");
        assert_eq!(env.env.get("KEY6").unwrap(), "removed_extended");
    }

    #[test]
    fn test_bundle_from_kdl_root() {
        let kdl_str = r#"
            bundle "zsh" { }

            bundle "neovim" { }
        "#;
        let kdl_doc: KdlDocument = kdl_str.parse().unwrap();
        let bundles = Bundle::from_kdl(&kdl_doc)
            .map_err(|err| miette::Error::new(err).with_source_code(kdl_str))
            .unwrap();
        assert_eq!(bundles.len(), 2);
        assert_eq!(bundles[0].name, "zsh");
        assert_eq!(bundles[0].dependencies.len(), 0);
        assert_eq!(bundles[0].dotfiles.len(), 0);
        assert_eq!(bundles[1].name, "neovim");
        assert_eq!(bundles[1].dependencies.len(), 0);
        assert_eq!(bundles[1].dotfiles.len(), 0);
    }

    #[test]
    fn test_bundle_from_kdl_dependencies() {
        let kdl_str = r#"
            bundle "zsh" {
                dependencies {
                    // zsh ">=3.8" // valid
                    zsh version=">=5.8" // valid
                    // zsh #true // error: invalid type on version
                    // zsh version=12345 // error: invalid type on version
                    // zsh version="invalid" // error: invalid version requirement

                    zoxide manager="cargo" // valid
                    // zoxide manager="unknown" // invalid
                }
            }

            bundle "neovim" { }
        "#;
        let kdl_doc: KdlDocument = kdl_str.parse().unwrap();
        let bundles = Bundle::from_kdl(&kdl_doc)
            .map_err(|err| miette::Error::new(err).with_source_code(kdl_str))
            .unwrap();
        let version_on_system = semver::Version::parse("10.0.0").unwrap();
        assert_eq!(bundles.len(), 2);
        assert_eq!(bundles[0].dependencies.len(), 2);
        assert_eq!(bundles[0].dependencies[0].name, "zsh");
        let version_req = bundles[0].dependencies[0].version.as_ref().unwrap();
        assert!(
            version_req.matches(&version_on_system),
            "required: {}, found: {}",
            version_req,
            version_on_system
        );
        assert_eq!(bundles[0].dependencies[1].name, "zoxide");
        assert!(bundles[0].dependencies[1].version.is_none());
        assert_eq!(bundles[0].dependencies[1].manager, Some(PackageManager::RustCargo));
    }
}
