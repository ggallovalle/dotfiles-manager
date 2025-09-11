use crate::config::env::EnvironmentVariables;
use crate::config::root::Options;
use crate::config::root::PackageManager;
use std::{fmt::Display, path::PathBuf};

use miette::{Diagnostic, LabeledSpan, NamedSource, SourceCode};
use thiserror::Error;

#[derive(Debug, Error, Diagnostic, Clone)]
pub enum ConfigError {
    #[error("Deserialization error: {0}")]
    KdlDeserializationError(#[from] kdl::KdlError),
    #[error("KdlDeserialization error: {0}")]
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

    fn code<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new("dots::config_kdl_error"))
    }

    fn url<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        Some(Box::new(
            "https://github.com/ggallovalle/blob/main/dotfiles-manager/documentation/configuraton.md",
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

macro_rules! kdl_parsing_error {
    ( $message:expr, $entry:expr ) => {
        ConfigError::new_kdl_error($message, $entry.span().offset(), $entry.span().len())
    };
    ( $message:expr, $entry:expr, $help: expr ) => {
        kdl_parsing_error!($message, $entry).set_help_message($help)
    };
}

macro_rules! kdl_node_required {
    ( $kdl_node:expr, $property_name:expr ) => {
        kdl_parsing_error!(format!("node {} is required", $property_name), $kdl_node)
    };
}

macro_rules! kdl_node_first_arg_as_string_or_error {
    ( $kdl_node:expr, $property_name:expr ) => {{
        match $kdl_node.get($property_name) {
            None => None,
            Some(node) => kdl_entry_string_or_error!(node, 0),
        }
    }};
}

macro_rules! kdl_entry_key_value_or_error {
    ( $node:expr ) => {{
        let mut entries = $node.entries().iter();
        let node_name = $node.name().value();
        match entries.next() {
            None => {
                None
                // return Err(kdl_parsing_error!(
                //     format!("node {} must have a key and a value", node_name),
                //     $node
                // ));
            }
            Some(key_entry) => match key_entry.name() {
                Some(name) => Some((name.value(), key_entry.value(), key_entry)),
                None => match entries.next() {
                    None => {
                        None
                        // return Err(kdl_parsing_error!(
                        //     format!("node {} must have a key and a value", node_name),
                        //     $node
                        // ));
                    }
                    Some(value_entry) => {
                        if !key_entry.value().is_string() {
                            return Err(kdl_parsing_error!(
                                format!("node {} must have a string key", node_name),
                                key_entry
                            ));
                        }
                        let key = key_entry.value().as_string().unwrap();
                        Some((key, value_entry.value(), value_entry))
                    }
                },
            },
        }
    }};
}

macro_rules! kdl_entry_key_value_string_or_error {
    (  $node:expr ) => {
        match kdl_entry_key_value_or_error!($node) {
            None => None,
            Some((key, value, value_entry)) => match value {
                kdl::KdlValue::String(string_value) => Some((key, string_value, value_entry)),
                _ => {
                    return Err(kdl_parsing_error!(
                        format!(
                            "the value in node {} with key {} must be a string",
                            $node.name().value(),
                            key
                        ),
                        $node
                    ));
                }
            },
        }
    };
}

macro_rules! kdl_entry_string_or_error {
    ( $kdl_node:expr, $entry:expr ) => {{
        {
            let node_key: kdl::NodeKey = $entry.into();
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
                                $entry
                            ),
                            entry
                        ));
                    }
                    Some(string_value) => Some((string_value, entry)),
                },
            }
        }
    }};
}

macro_rules! kdl_entry_string {
    ( $kdl_node:expr, $entry:expr ) => {{
        {
            let node_key: kdl::NodeKey = $entry.into();
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
    ( $kdl_node:expr, $entry:expr ) => {{
        {
            let node_key: kdl::NodeKey = $entry.into();
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
                                "node {} entry {} {} must be a boolean",
                                $kdl_node.name().value(),
                                entry_type,
                                $entry
                            ),
                            entry
                        ));
                    }
                    Some(bool_value) => Some((bool_value, entry)),
                },
            }
        }
    }};
}

macro_rules! kdl_entry_bool {
    ( $kdl_node:expr, $entry:expr ) => {{
        {
            let node_key: kdl::NodeKey = $entry.into();
            match $kdl_node.entry(node_key) {
                None => None,
                Some(entry) => match entry.value().as_bool() {
                    None => None,
                    Some(bool_value) => Some((bool_value, entry)),
                },
            }
        }
    }};
}

impl Options {
    pub fn from_kdl(kdl_doc: &kdl::KdlDocument) -> Result<Self, ConfigError> {
        let dotfiles_dir = kdl_node_first_arg_as_string_or_error!(kdl_doc, "dotfiles_dir")
            .map(|(v, _)| PathBuf::from(v))
            .ok_or(kdl_node_required!(kdl_doc, "dotfiles_dir"))?;

        let mut options = Options::create(dotfiles_dir);
        let mut package_managers = vec![];
        match kdl_doc.get("package_managers") {
            None => return Err(kdl_node_required!(kdl_doc, "package_managers")),
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
    pub fn apply_kdl(&mut self, kdl_doc: &kdl::KdlDocument) -> Result<(), ConfigError> {
        for node in kdl_doc.nodes().iter().filter(|n| n.name().value() == "env") {
            if let Some((inherit, _e)) = kdl_entry_bool_or_error!(node, "inherit")
                && let Some((key, _e)) = kdl_entry_string!(node, 0)
            {
                if inherit && let Ok(system_value) = std::env::var(&key) {
                    self.env.insert(key.to_string(), system_value);
                } else {
                    self.env.shift_remove(&key.to_string());
                }
                continue;
            }

            if let Some((key, value, _e)) = kdl_entry_key_value_string_or_error!(node) {
                self.env.insert(key.to_string(), self.expand(&value));
            }
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
            // env HOME inherit=12345 // error: inherit must be boolean
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
