use crate::{
    diag,
    env::{self, Env},
    impl_from_kdl_entry_for_enum,
};
use indexmap::IndexMap;
use kdl::{KdlDiagnostic, KdlDocument, KdlEntry, KdlNode, KdlValue};
use kdl_helpers::{self as h, FromKdlEntry, KdlDocumentExt};
use miette::{Severity, SourceSpan};
use std::path::PathBuf;
use strum::{self, VariantNames};

mod error;
mod kdl_helpers;

pub use error::{ConfigDiagnostic, ConfigError, OneOf};
pub use kdl_helpers::KdlItemRef;

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Env,
    pub dotfiles_dir: PathBuf,
    pub bundles: IndexMap<String, Vec<BundleItem>>,
    bundles_envs: IndexMap<String, Env>,
}

#[derive(Debug, Clone)]
pub enum BundleItem {
    Copy { source: PathBuf, target: PathBuf, span: SourceSpan },
    Link { source: PathBuf, target: PathBuf, span: SourceSpan },
    Alias { from: String, to: String, span: SourceSpan },
    Source { snippet: String, position: Position, shell: Shell, span: SourceSpan },
}

#[derive(Debug, Clone, PartialEq, strum::EnumString, strum::VariantNames)]
pub enum Shell {
    #[strum(serialize = "bash")]
    Bash,
    #[strum(serialize = "zsh")]
    Zsh,
    #[strum(serialize = "fish")]
    Fish,
    #[strum(serialize = "pwsh")]
    PowerShell,
    #[strum(serialize = "nushell")]
    Nushell,
    #[strum(transparent)]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, strum::EnumString, strum::VariantNames)]
pub enum Position {
    #[strum(serialize = "start")]
    Start,
    #[strum(serialize = "end")]
    End,
    #[strum(serialize = "random")]
    Random,
}

impl_from_kdl_entry_for_enum!(Shell);
impl_from_kdl_entry_for_enum!(Position);

impl Config {
    pub fn get_env_for_bundle<T: AsRef<str>>(&self, bundle: T) -> &Env {
        self.bundles_envs.get(bundle.as_ref()).unwrap_or(&self.env)
    }

    pub fn from_kdl(document: KdlDocument) -> Result<Self, ConfigDiagnostic> {
        let mut env_map = Env::empty();
        env_map.apply_xdg();
        let dotfiles_dir = document
            .get_node_required_one("dotfiles_dir")
            .and_then(h::arg0)
            .map_err(Into::into)
            .and_then(|entry| env_map.expand_kdl_entry_dir_exists(entry))?;

        env_map.apply_exports_to_env(&document)?;

        let mut bundles: IndexMap<String, Vec<BundleItem>> = IndexMap::new();
        let mut bundles_envs = IndexMap::new();

        for bundle in document.get_children_named("bundle") {
            let bundle_name = h::arg0(bundle).and_then(String::from_kdl_entry)?;
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
            let mut items = Vec::new();
            let mut env_for_bundle: Option<Env> = None;

            for bundle_item in bundle.get_children() {
                let current_env_map = env_for_bundle.as_ref().unwrap_or(&env_map);
                match bundle_item.name().value() {
                    "alias" => {
                        let (key, value) = h::prop0(bundle_item)?;
                        items.push(BundleItem::Alias {
                            from: key.value().to_string(),
                            to: String::from_kdl_entry(value)?,
                            span: bundle_item.span(),
                        });
                    }
                    "cp" => {
                        let source_entry = h::arg0(bundle_item)?;
                        let source = dotfiles_dir.join(String::from_kdl_entry(source_entry)?);
                        if !source.exists() {
                            return Err(ConfigDiagnostic::path_not_found(
                                source_entry,
                                source.display().to_string(),
                            ));
                        }
                        let target = h::arg(bundle_item, 1)
                            .map_err(Into::into)
                            .and_then(|entry| current_env_map.expand_kdl_entry(entry))?;
                        // let target_path = if target.replacement_count > 0 {
                        //     PathBuf::from(&target.value)
                        // } else {
                        //     let path = PathBuf::from(env_map.get("HOME").unwrap());
                        //     path.join(&target.value)
                        // };
                        let target_path = PathBuf::from(&target);
                        items.push(BundleItem::Copy {
                            source,
                            target: target_path,
                            span: bundle_item.span(),
                        });
                    }
                    "ln" => {
                        // same as cp but creates a symlink instead of copying
                        let source_entry = h::arg0(bundle_item)?;
                        let source = dotfiles_dir.join(String::from_kdl_entry(source_entry)?);
                        if !source.exists() {
                            return Err(ConfigDiagnostic::path_not_found(
                                source_entry,
                                source.display().to_string(),
                            ));
                        }
                        let target = h::arg(bundle_item, 1)
                            .map_err(Into::into)
                            .and_then(|entry| current_env_map.expand_kdl_entry(entry))?;
                        // let target_path = if target.replacement_count > 0 {
                        //     PathBuf::from(&target.value)
                        // } else {
                        //     let path = PathBuf::from(env_map.get("HOME").unwrap());
                        //     path.join(&target.value)
                        // };
                        let target_path = PathBuf::from(&target);
                        items.push(BundleItem::Link {
                            source,
                            target: target_path,
                            span: bundle_item.span(),
                        });
                    }
                    "source" => {
                        let snippet = h::arg(bundle_item, 0).and_then(String::from_kdl_entry)?;
                        let shell = h::prop(bundle_item, "shell")
                            .map_err(Into::into)
                            .and_then(Shell::from_kdl_entry)?;

                        let position = bundle_item
                            .entry("position")
                            .map_or(Ok(Position::Random), |e| Position::from_kdl_entry(e))?;
                        items.push(BundleItem::Source {
                            snippet,
                            position,
                            shell,
                            span: bundle_item.span(),
                        });
                    }
                    Env::ENV_NODE => match env_for_bundle.as_mut() {
                        Some(e) => {
                            e.apply_node(bundle_item)?;
                        }
                        None => {
                            let mut e = env_map.child();
                            e.apply_node(bundle_item)?;
                            env_for_bundle = Some(e);
                        }
                    },
                    _ => {
                        return Err(ConfigDiagnostic::unknown_variant(
                            bundle_item,
                            bundle_item.name().value(),
                            OneOf::from_iter(&[
                                // "install",
                                "cp",
                                "ln",
                                "alias",
                                "source",
                                Env::ENV_NODE,
                            ]),
                        ));
                    }
                }
            }

            if let Some(e) = env_for_bundle {
                bundles_envs.insert(bundle_name.clone(), e);
            }
            bundles.insert(bundle_name, items);
        }

        Ok(Config { env: env_map, dotfiles_dir, bundles, bundles_envs })
    }
}

impl Env {
    const ENV_NODE: &str = "env";

    fn apply_exports_to_env(
        &mut self,
        document: &impl h::KdlDocumentExt,
    ) -> Result<(), ConfigDiagnostic> {
        for node in document.get_children_named(Self::ENV_NODE) {
            self.apply_node(node)?;
        }
        Ok(())
    }

    fn apply_node(&mut self, node: &KdlNode) -> Result<(), ConfigDiagnostic> {
        let mode_or_env_kdl = h::entry_at(node, 0)?;
        match mode_or_env_kdl.name() {
            Some(id) => {
                // env KEY=VALUE
                // no export or import, just set the value
                let env_kdl = mode_or_env_kdl;
                let v = self.expand_kdl_entry(env_kdl)?;
                self.insert(
                    id.value().to_string(),
                    env::EnvValue::String(v),
                    env::EnvItemMeta {
                        inherited: false,
                        exported: false,
                        span: Some(env_kdl.into()),
                    },
                );
            }
            None => {
                let mode_kdl = mode_or_env_kdl;
                let mode = h::as_str(mode_kdl)?;
                match mode {
                    "export" => {
                        // env export KEY=VALUE
                        let (env_key, env_kdl) = h::prop_at(node, 1)?;
                        let v = self.expand_kdl_entry(env_kdl)?;
                        self.insert(
                            env_key.value().to_string(),
                            env::EnvValue::String(v),
                            env::EnvItemMeta {
                                inherited: false,
                                exported: true,
                                span: Some(env_kdl.into()),
                            },
                        );
                    }
                    "import" => {
                        let env_kdl = h::entry_at(node, 1)?;
                        let (key, default) = match env_kdl.name() {
                            Some(id) => {
                                let default = h::as_str(env_kdl)?;
                                (id.value(), if default.is_empty() { None } else { Some(default) })
                            }
                            None => (h::as_str(env_kdl)?, None),
                        };
                        match (std::env::var(key), default) {
                            (Ok(value), _) => {
                                self.insert(
                                    key.to_string(),
                                    env::EnvValue::String(value),
                                    env::EnvItemMeta {
                                        inherited: false,
                                        exported: true,
                                        span: Some(env_kdl.into()),
                                    },
                                );
                            }
                            (Err(std::env::VarError::NotPresent), None) => {
                                return Err(diag!(
                                        env_kdl.span(),
                                        message = format!(
                                            "environment variable '{}' not found",
                                            key
                                        ),
                                        help = "ensure the environment variable is set, or provide a default value",
                                        severity = Severity::Warning
                                    )
                                    .into());
                            }
                            (Err(std::env::VarError::NotPresent), Some(def)) => {
                                self.insert(
                                    key.to_string(),
                                    env::EnvValue::String(def.to_string()),
                                    env::EnvItemMeta {
                                        inherited: false,
                                        exported: false,
                                        span: Some(env_kdl.into()),
                                    },
                                );
                            }
                            (Err(std::env::VarError::NotUnicode(value)), _) => {
                                return Err(diag!(
                                    env_kdl.span(),
                                    message = format!(
                                        "environment variable '{}' is not valid unicode: {:?}",
                                        key, value
                                    ),
                                    help = "ensure the environment variable is valid unicode",
                                    severity = Severity::Warning
                                )
                                .into());
                            }
                        }
                    }
                    _ => {
                        return Err(ConfigDiagnostic::unknown_variant(
                            mode_kdl,
                            mode,
                            OneOf::from_iter(&["export", "import"]),
                        ));
                    }
                }
            }
        };

        Ok(())
    }

    fn expand_kdl_entry(&self, entry: &KdlEntry) -> Result<String, ConfigDiagnostic> {
        match self.expand(&String::from_kdl_entry(entry)?) {
            Err(e) => {
                Err(ConfigDiagnostic::env_expand_error(entry, e, OneOf::from_iter(self.keys())))
            }
            Ok(expanded) => Ok(expanded),
        }
    }

    pub fn expand_kdl_entry_dir_exists(
        &self,
        entry: &KdlEntry,
    ) -> Result<PathBuf, ConfigDiagnostic> {
        let expanded = self.expand_kdl_entry(entry)?;
        let path = PathBuf::from(&expanded);
        match path.metadata() {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(ConfigDiagnostic::path_not_found(entry, path.display().to_string()));
            }
            Err(err) => {
                return Err(diag!(
                    entry.span(),
                    message = format!("failed to access path {}: {}", expanded, err),
                    help =
                        "check the path permissions or system state, or update the configuration",
                    severity = Severity::Warning
                )
                .into());
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(diag!(
                    entry.span(),
                    message = format!("path is not a directory: {}", expanded),
                    help = "ensure the path is a directory, or update the configuration",
                    severity = Severity::Warning
                )
                .into());
            }
            Ok(_) => { /* path exists and is a directory */ }
        }
        Ok(path)
    }
}
