use crate::{
    diag,
    env::{self, ExpandValue},
    impl_from_kdl_entry_for_enum,
    kdl_helpers::{self as h, FromKdlEntry, KdlDocumentExt},
    package_manager::{self, ManagerIdentifier},
    settings_error::{OneOf, SettingsDiagnostic},
};
use indexmap::IndexMap;
use kdl::{KdlDiagnostic, KdlDocument, KdlEntry, KdlNode, KdlValue};
use miette::{Severity, SourceSpan};
use semver::{self, VersionReq};
use std::path::PathBuf;
use strum::{self, VariantNames};

#[derive(Debug, Clone)]
pub struct Settings {
    pub env: IndexMap<String, String>,
    env_inherited_keys: Vec<String>,
    pub dotfiles_dir: PathBuf,
    pub package_managers: IndexMap<ManagerIdentifier, PathBuf>,
    pub bundles: IndexMap<String, Vec<BundleItem>>,
}

#[derive(Debug, Clone)]
pub enum BundleItem {
    Install {
        name: String,
        manager: Option<ManagerIdentifier>,
        version: Option<semver::VersionReq>,
        span: SourceSpan,
    },
    Copy {
        source: PathBuf,
        target: PathBuf,
        span: SourceSpan,
    },
    Link {
        source: PathBuf,
        target: PathBuf,
        span: SourceSpan,
    },
    Alias {
        from: String,
        to: String,
        span: SourceSpan,
    },
    Clone {
        repo: String,
        target: PathBuf,
        span: SourceSpan,
    },
    Source {
        snippet: String,
        position: Position,
        shell: Shell,
        span: SourceSpan,
    },
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
    #[strum(serialize = "other")]
    Other,
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
impl_from_kdl_entry_for_enum!(ManagerIdentifier);

impl Settings {
    pub fn from_kdl(document: KdlDocument) -> Result<Self, SettingsDiagnostic> {
        let mut env_map = env::base();
        let env_inherited_keys = env_map.keys().cloned().collect::<Vec<_>>();
        let dotfiles_dir = document
            .get_node_required_one("dotfiles_dir")
            .and_then(h::arg0)
            .map_err(Into::into)
            .and_then(|entry| env::ExpandValue::from_kdl_entry_dir_exists(entry, &env_map))?;

        let package_managers_node = document.get_node_required_one("package_managers")?;
        let mut package_managers = IndexMap::new();
        for manager_entry in h::args(package_managers_node)? {
            let manager = ManagerIdentifier::from_kdl_entry(manager_entry)?;
            match manager.which() {
                Some(path) => {
                    if let Some(_) = package_managers.insert(manager.clone(), path) {
                        return Err(diag!(
                            manager_entry.span(),
                            message =
                                format!("package manager '{}' can only be specified once", manager)
                        )
                        .into());
                    }
                }
                None => {
                    return Err(diag!(
                        manager_entry.span(),
                        message =
                            format!("package manager '{}' not found in PATH", manager.to_string()),
                        help = "ensure the package manager is installed and available in PATH",
                        severity = Severity::Warning
                    )
                    .into());
                }
            }
        }

        env::ExpandValue::apply_exports_to_env(&document, &mut env_map)?;

        let mut bundles: IndexMap<String, Vec<BundleItem>> = IndexMap::new();

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
            env::ExpandValue::apply_exports_to_env(bundle, &mut env_map)?;
            let mut items = Vec::new();

            for bundle_item in bundle.get_children() {
                match bundle_item.name().value() {
                    "install" => {
                        let name = h::arg0(bundle_item).and_then(String::from_kdl_entry)?;
                        let manager = bundle_item
                            .entry("pm")
                            .map(ManagerIdentifier::from_kdl_entry_keep)
                            .transpose()?;
                        if let Some((mgr_entry, ref mgr)) = manager
                            && !package_managers.contains_key(mgr)
                        {
                            return Err(SettingsDiagnostic::unknown_variant_reference(
                                mgr_entry,
                                mgr.to_string(),
                                OneOf::from_iter(package_managers.keys()),
                                package_managers_node,
                            ));
                        }
                        let version = bundle_item
                            .entry("version")
                            .map(VersionReq::from_kdl_entry)
                            .transpose()?;
                        items.push(BundleItem::Install {
                            name,
                            manager: manager.map(|(_, m)| m),
                            version,
                            span: bundle_item.span(),
                        });
                    }
                    "alias" => {
                        let (key, value) = h::prop0(bundle_item)?;
                        items.push(BundleItem::Alias {
                            from: key.value().to_string(),
                            to: String::from_kdl_entry(value)?,
                            span: bundle_item.span(),
                        });
                    }
                    "clone" => {
                        let repo = h::arg0(bundle_item).and_then(String::from_kdl_entry)?;
                        let target = h::arg(bundle_item, 1)
                            .map_err(Into::into)
                            .and_then(|entry| env::ExpandValue::from_kdl_entry(entry, &env_map))?;
                        items.push(BundleItem::Clone {
                            repo,
                            target: PathBuf::from(target.value),
                            span: bundle_item.span(),
                        });
                    }
                    "cp" => {
                        let source_entry = h::arg0(bundle_item)?;
                        let source = dotfiles_dir.join(String::from_kdl_entry(source_entry)?);
                        if !source.exists() {
                            return Err(SettingsDiagnostic::path_not_found(
                                source_entry,
                                source.display().to_string(),
                            ));
                        }
                        let target = h::arg(bundle_item, 1)
                            .map_err(Into::into)
                            .and_then(|entry| env::ExpandValue::from_kdl_entry(entry, &env_map))?;
                        items.push(BundleItem::Copy {
                            source,
                            target: PathBuf::from(target.value),
                            span: bundle_item.span(),
                        });
                    }
                    "ln" => {
                        // same as cp but creates a symlink instead of copying
                        let source_entry = h::arg0(bundle_item)?;
                        let source = dotfiles_dir.join(String::from_kdl_entry(source_entry)?);
                        if !source.exists() {
                            return Err(SettingsDiagnostic::path_not_found(
                                source_entry,
                                source.display().to_string(),
                            ));
                        }
                        let target = h::arg(bundle_item, 1)
                            .map_err(Into::into)
                            .and_then(|entry| env::ExpandValue::from_kdl_entry(entry, &env_map))?;
                        items.push(BundleItem::Link {
                            source,
                            target: PathBuf::from(target.value),
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
                    "export" => { /* already handled */ }
                    _ => {
                        return Err(SettingsDiagnostic::unknown_variant(
                            bundle_item,
                            bundle_item.name().value(),
                            OneOf::from_iter(&[
                                "install", "cp", "ln", "alias", "clone", "source", "export",
                            ]),
                        ));
                    }
                }
            }

            bundles.insert(bundle_name, items);
        }

        Ok(Settings { env: env_map, env_inherited_keys, dotfiles_dir, bundles, package_managers })
    }
}

impl env::ExpandValue {
    fn apply_exports_to_env(
        document: &impl h::KdlDocumentExt,
        env_map: &mut IndexMap<String, String>,
    ) -> Result<(), SettingsDiagnostic> {
        for n in document.get_children_named("export") {
            let (id, entry) = h::prop0(n)?;
            let v = env::ExpandValue::from_kdl_entry(entry, env_map)?;
            env_map.insert(id.value().to_string(), v.value);
        }
        Ok(())
    }

    fn from_kdl_entry(
        entry: &KdlEntry,
        env: &IndexMap<String, String>,
    ) -> Result<Self, SettingsDiagnostic> {
        match env::expand(&String::from_kdl_entry(entry)?, env) {
            Err(e) => {
                Err(SettingsDiagnostic::unknown_variant(entry, e.var, OneOf::from_iter(env.keys())))
            }
            Ok(expanded) => Ok(expanded),
        }
    }

    pub fn from_kdl_entry_dir_exists(
        entry: &KdlEntry,
        env: &IndexMap<String, String>,
    ) -> Result<PathBuf, SettingsDiagnostic> {
        let expanded = Self::from_kdl_entry(entry, env)?;
        let path = PathBuf::from(&expanded.value);
        match path.metadata() {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(SettingsDiagnostic::path_not_found(entry, path.display().to_string()));
            }
            Err(err) => {
                return Err(diag!(
                    entry.span(),
                    message = format!("failed to access path {}: {}", expanded.value, err),
                    help =
                        "check the path permissions or system state, or update the configuration",
                    severity = Severity::Warning
                )
                .into());
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(diag!(
                    entry.span(),
                    message = format!("path is not a directory: {}", expanded.value),
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
