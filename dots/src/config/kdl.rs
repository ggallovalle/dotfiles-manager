use crate::config::env::EnvironmentVariables;
use crate::config::root::Options;
use crate::config::root::PackageManager;
use std::{fmt::Display, path::PathBuf};

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceCode};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    #[error("Deserialization error: {0}")]
    KdlDeserializationError(#[from] kdl::KdlError),
    #[error("KdlDeserialization error: {0}")]
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

#[derive(Error, Debug)]
pub struct KdlError {
    pub error_message: String,
    pub src: Option<NamedSource<String>>,
    pub offset: Option<usize>,
    pub len: Option<usize>,
    pub help_message: Option<String>,
}

impl Display for KdlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "Failed to parse Dots configuration")
    }
}

impl Diagnostic for KdlError {
    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self.src.as_ref() {
            Some(src) => Some(src),
            None => None,
        }
    }
    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        match &self.help_message {
            Some(help_message) => Some(Box::new(help_message)),
            None => Some(Box::new(format!(
                "For more information, please see our configuration guide: https://zellij.dev/documentation/configuration.html"
            ))),
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

macro_rules! kdl_parsing_error {
    ( $message:expr, $entry:expr ) => {
        ConfigError::new_kdl_error($message, $entry.span().offset(), $entry.span().len())
    };
    ( $message:expr, $entry:expr, $help: expr ) => {
        kdl_parsing_error!($message, $entry).set_help_message($help)
    };
}

macro_rules! kdl_property_required {
    ( $kdl_node:expr, $property_name:expr ) => {
        kdl_parsing_error!(format!("Property {} is required", $property_name), $kdl_node)
    };
}

macro_rules! kdl_property_first_arg_as_string_or_error {
    ( $kdl_node:expr, $property_name:expr ) => {{
        match $kdl_node.get($property_name) {
            Some(property) => match property.entries().iter().next() {
                Some(first_entry) => match first_entry.value().as_string() {
                    Some(string_entry) => Some((string_entry, first_entry)),
                    None => {
                        return Err(kdl_parsing_error!(
                            format!(
                                "Property {} must be a string, found: {}",
                                $property_name,
                                first_entry.value()
                            ),
                            property
                        ));
                    }
                },
                None => {
                    return Err(kdl_parsing_error!(
                        format!("Property {} must have a value", $property_name),
                        property
                    ));
                }
            },
            None => None,
        }
    }};
}
macro_rules! kdl_entry_key_value_or_error {
    (  $node:expr, $property_name:expr ) => {{
        let mut entries = $node.entries().iter();
        match entries.next() {
            None => {
                return Err(kdl_parsing_error!(
                    format!("node {} must have a key and a value", $property_name),
                    $node
                ));
            }
            Some(entry) => match entry.name() {
                Some(name) => (name.value().to_string(), entry.value()),
                None => match entries.next() {
                    None => {
                        return Err(kdl_parsing_error!(
                            format!("node {} must have a key and a value", $property_name),
                            $node
                        ));
                    }
                    Some(value_entry) => {
                        if !entry.value().is_string() {
                            return Err(kdl_parsing_error!(
                                format!("the key in node {} must be a string", $property_name),
                                entry
                            ));
                        }
                        let key = entry.value().as_string().unwrap().to_string();
                        (key, value_entry.value())
                    }
                },
            },
        }
    }};
}

macro_rules! kdl_entry_key_value_string_or_error {
    (  $node:expr, $property_name:expr ) => {
        match kdl_entry_key_value_or_error!($node, $property_name) {
            (key, value) => match value {
                kdl::KdlValue::String(string_value) => (key, string_value),
                _ => {
                    return Err(kdl_parsing_error!(
                        format!(
                            "the value in node {} with key {} must be a string",
                            $property_name, key
                        ),
                        $node
                    ));
                }
            },
        }
    };
}

macro_rules! kdl_entry_string_or_error {
    ( $kdl_node:expr, $prop:expr ) => {{
        {
            let node_key: kdl::NodeKey = $prop.into();
            let entry_type = match node_key {
                kdl::NodeKey::Index(_) => "argument",
                kdl::NodeKey::Key(_) => "property",
            };
            match $kdl_node.entry(node_key) {
                None => None,
                Some(entry) => match entry.value().as_string() {
                    None => {
                        return Err(kdl_parsing_error!(
                            format!(
                                "node {} {} {} must be a string",
                                $kdl_node.name().value(),
                                entry_type,
                                $prop
                            ),
                            entry
                        ));
                    }
                    Some(string_value) => Some(string_value),
                },
            }
        }
    }};
}

macro_rules! kdl_entry_string {
    ( $kdl_node:expr, $prop:expr ) => {{
        {
            let node_key: kdl::NodeKey = $prop.into();
            match $kdl_node.entry(node_key) {
                None => None,
                Some(entry) => match entry.value().as_string() {
                    None => None,
                    Some(string_value) => Some((string_value, entry)),
                },
            }
        }
    }};
}

macro_rules! kdl_entry_bool_or_error {
    ( $kdl_node:expr, $prop:expr ) => {{
        {
            let node_key: kdl::NodeKey = $prop.into();
            let entry_type = match node_key {
                kdl::NodeKey::Index(_) => "argument",
                kdl::NodeKey::Key(_) => "property",
            };
            match $kdl_node.entry(node_key) {
                None => None,
                Some(entry) => match entry.value().as_bool() {
                    None => {
                        return Err(kdl_parsing_error!(
                            format!(
                                "node {} {} {} must be a boolean",
                                $kdl_node.name().value(),
                                entry_type,
                                $prop
                            ),
                            entry
                        ));
                    }
                    Some(bool_value) => Some(bool_value),
                },
            }
        }
    }};
}

impl Options {
    pub fn from_kdl(kdl_options: &kdl::KdlDocument) -> Result<Self, ConfigError> {
        let dotfiles_dir = kdl_property_first_arg_as_string_or_error!(kdl_options, "dotfiles_dir")
            .map(|(v, _)| PathBuf::from(v))
            .ok_or(kdl_property_required!(kdl_options, "dotfiles_dir"))?;

        let mut options = Options::create(dotfiles_dir);
        let mut package_managers = vec![];
        match kdl_options.get("package_managers") {
            None => return Err(kdl_property_required!(kdl_options, "package_managers")),
            Some(node) => {
                let available_in_system =
                    options.package_manager.keys().cloned().collect::<Vec<_>>();

                for entry in node.entries().iter() {
                    if let Some(entry_name) = entry.name() {
                        return Err(kdl_parsing_error!(
                            format!(
                                "Each entry in package_managers must be an argument, found property: {}",
                                entry_name.value()
                            ),
                            entry_name,
                            format!("write it as an argument, not a property.")
                        ));
                    }
                    match entry.value().as_string() {
                        Some(string_entry) => match string_entry.parse::<PackageManager>() {
                            Ok(package_manager) => {
                                if !available_in_system.contains(&package_manager) {
                                    return Err(kdl_parsing_error!(
                                        format!(
                                            "Package manager {} is not available on this system",
                                            package_manager.to_string()
                                        ),
                                        entry,
                                        format!(
                                            "detected package managers on this system are: {}",
                                            available_in_system
                                                .iter()
                                                .map(|pm| pm.to_string())
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        )
                                    ));
                                }
                                package_managers.push(package_manager);
                            }
                            Err(valid_names) => {
                                return Err(kdl_parsing_error!(
                                    format!("Invalid package manager: {}", string_entry,),
                                    entry,
                                    format!("Valid options are: {}", valid_names.join(", "))
                                ));
                            }
                        },
                        None => {
                            return Err(kdl_parsing_error!(
                                "Each entry in package_managers must be a string".into(),
                                entry
                            ));
                        }
                    }
                }
                if package_managers.is_empty() {
                    return Err(kdl_parsing_error!(
                        "package_managers can not be empty".into(),
                        node,
                        "add at least one element to the list".into()
                    ));
                }
            }
        }
        options.package_manager.retain(|pm, _| package_managers.contains(pm));

        Ok(options)
    }
}

impl EnvironmentVariables {
    pub fn apply_kdl(&mut self, kdl_env: &kdl::KdlDocument) -> Result<(), ConfigError> {
        for node in kdl_env.nodes().iter().filter(|n| n.name().value() == "env") {
            if let Some(inherit) = kdl_entry_bool_or_error!(node, "inherit")
                && let Some((key, _)) = kdl_entry_string!(node, 0)
            {
                if inherit && let Ok(system_value) = std::env::var(&key) {
                    self.env.insert(key.to_string(), system_value);
                } else {
                    self.env.shift_remove(&key.to_string());
                }
                continue;
            }

            let (key, value) = kdl_entry_key_value_string_or_error!(node, "env");
            self.env.insert(key, self.expand(&value));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(input: &str) -> kdl::KdlDocument {
        input.parse().unwrap()
    }

    #[test]
    fn test_options_from_kdl() {
        let kdl_str = r#"
            dotfiles_dir "/home/user/dotfiles"
            package_managers cargo // comment to fail with "property required" error

            // package_managers key="cargo" // fail with "no properties allowed" error

            // package_managers "unknown_pm" // fail with "invalid package manager" error

            // package_managers "winget" // fail with "not available on this system" error

            // package_managers #true // fail with "must be a string" error
            // package_managers // fail with "canot be empty" error
        "#;
        let kdl_doc = document(kdl_str);
        let options = Options::from_kdl(&kdl_doc).unwrap();
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
            env GIBRISH inherit=#true
            env GIBRISH_1 inherit=#false
            env PLAIN inherit=#false
            env KEY1 "value1"
            env KEY2 "${KEY1}_suffix"
            env KEY3="${KEY2}_more"
            env KEY4="${GIBRISH}_extended"
            env KEY5 "${GIBRISH_1:-not-inherited}_extended"
            env KEY6 "${PLAIN:-removed}_extended"

            // hello "ignored"

            // env HOME inherit=12345 // error: inherit must be boolean
        "#;
        let kdl_doc = document(kdl_str);
        let mut env = EnvironmentVariables::default();
        env.env.insert("PLAIN".to_string(), "im-plain".to_string());
        env.apply_kdl(&kdl_doc).unwrap();
        assert_eq!(env.env.get("KEY1").unwrap(), "value1");
        assert_eq!(env.env.get("KEY2").unwrap(), "value1_suffix");
        assert_eq!(env.env.get("KEY3").unwrap(), "value1_suffix_more");
        assert_eq!(env.env.get("KEY4").unwrap(), "gibrish_extended");
        assert_eq!(env.env.get("KEY5").unwrap(), "not-inherited_extended");
        assert_eq!(env.env.get("KEY6").unwrap(), "removed_extended");
    }
}
