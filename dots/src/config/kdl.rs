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
        ConfigError::new_kdl_error(
            format!("Property {} is required", $property_name),
            $kdl_node.span().offset(),
            $kdl_node.span().len(),
        )
    };
}

macro_rules! kdl_property_first_arg_as_string_or_error {
    ( $kdl_node:expr, $property_name:expr ) => {{
        match $kdl_node.get($property_name) {
            Some(property) => match property.entries().iter().next() {
                Some(first_entry) => match first_entry.value().as_string() {
                    Some(string_entry) => Some((string_entry, first_entry)),
                    None => {
                        return Err(ConfigError::new_kdl_error(
                            format!(
                                "Property {} must be a string, found: {}",
                                $property_name,
                                first_entry.value()
                            ),
                            property.span().offset(),
                            property.span().len(),
                        ));
                    }
                },
                None => {
                    return Err(ConfigError::new_kdl_error(
                        format!("Property {} must have a value", $property_name),
                        property.span().offset(),
                        property.span().len(),
                    ));
                }
            },
            None => None,
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
}
