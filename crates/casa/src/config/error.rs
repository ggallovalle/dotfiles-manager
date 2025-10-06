use super::kdl_helpers::KdlItemRef;
use kdl;
use miette::Diagnostic;
use miette::LabeledSpan;
use miette::SourceSpan;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ConfigError {
    pub input: Arc<String>,
    pub diagnostics: Vec<ConfigDiagnostic>,
    named_source: Option<miette::NamedSource<Arc<String>>>, // store it here to support Diagnostic::source_code
}

impl ConfigError {
    pub fn from_str(input: impl Into<Arc<String>>, diagnostics: Vec<ConfigDiagnostic>) -> Self {
        let input = input.into();
        ConfigError { input, diagnostics, named_source: None }
    }

    pub fn from_file(
        name: &PathBuf,
        input: Arc<String>,
        diagnostics: Vec<ConfigDiagnostic>,
    ) -> Self {
        let named_source = Some(
            miette::NamedSource::new(name.to_string_lossy(), input.clone()).with_language("kdl"),
        );
        ConfigError { input, diagnostics, named_source }
    }
}

impl ConfigError {
    pub fn diagnostics_jsonable(&self) -> Vec<HashMap<String, String>> {
        self.diagnostics
            .iter()
            .map(|d| {
                let mut map = HashMap::new();
                map.insert("message".to_string(), d.to_string());
                let span = d.span();
                map.insert("span".to_string(), format!("{}:+{}", span.offset(), span.len()));
                map.insert("kind".to_string(), d.kind().to_string());
                if let Some(help) = d.help() {
                    map.insert("help".to_string(), format!("{}", help));
                }
                if let Some(severity) = d.severity() {
                    map.insert("severity".to_string(), format!("{:?}", severity));
                }
                map
            })
            .collect()
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.named_source {
            write!(f, "config error in {}", name.name())
        } else {
            write!(f, "config error")
        }
    }
}

impl std::error::Error for ConfigError {}

impl Diagnostic for ConfigError {
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        if let Some(name) = &self.named_source { Some(name) } else { Some(&*self.input) }
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        Some(Box::new(self.diagnostics.iter().map(|d| d as &dyn Diagnostic)))
    }
}

#[derive(Debug, Clone)]
pub enum ConfigDiagnostic {
    UnknownVariant {
        variant: String,
        expected: OneOf,
        source: KdlItemRef,
        source_ref: Option<KdlItemRef>,
    },
    PathNotFound {
        path: String,
        source: KdlItemRef,
    },
    ParseError(kdl::KdlDiagnostic),
    EnvExpandError {
        error: subst::Error,
        source: KdlItemRef,
        expected: OneOf,
    },
}

impl ConfigDiagnostic {
    pub fn unknown_variant(
        source: impl Into<KdlItemRef>,
        variant: impl Into<String>,
        expected: impl Into<OneOf>,
    ) -> Self {
        ConfigDiagnostic::UnknownVariant {
            variant: variant.into(),
            expected: expected.into(),
            source: source.into(),
            source_ref: None,
        }
    }

    pub fn unknown_variant_reference(
        source: impl Into<KdlItemRef>,
        variant: impl Into<String>,
        expected: impl Into<OneOf>,
        source_ref: impl Into<KdlItemRef>,
    ) -> Self {
        ConfigDiagnostic::UnknownVariant {
            variant: variant.into(),
            expected: expected.into(),
            source: source.into(),
            source_ref: Some(source_ref.into()),
        }
    }

    pub fn path_not_found(source: impl Into<KdlItemRef>, path: impl Into<String>) -> Self {
        ConfigDiagnostic::PathNotFound { path: path.into(), source: source.into() }
    }

    pub fn env_expand_error(
        source: impl Into<KdlItemRef>,
        error: subst::Error,
        expected: impl Into<OneOf>,
    ) -> Self {
        ConfigDiagnostic::EnvExpandError { error, source: source.into(), expected: expected.into() }
    }
}

impl ConfigDiagnostic {
    pub fn span(&self) -> SourceSpan {
        match self {
            ConfigDiagnostic::UnknownVariant { source, .. } => source.span_value().clone(),
            ConfigDiagnostic::ParseError(error) => error.span.clone(),
            ConfigDiagnostic::PathNotFound { source, .. } => source.span_value().clone(),
            ConfigDiagnostic::EnvExpandError { source, .. } => source.span_value().clone(),
        }
    }

    fn miette_default_label(&self) -> LabeledSpan {
        LabeledSpan::new_primary_with_span(Some("here".to_owned()), self.span())
    }

    fn kind(&self) -> &str {
        match self {
            ConfigDiagnostic::UnknownVariant { source_ref, .. } if source_ref.is_some() => {
                "unknown variant reference"
            }
            ConfigDiagnostic::UnknownVariant { .. } => "unknown variant",
            ConfigDiagnostic::ParseError(_) => "parse error",
            ConfigDiagnostic::PathNotFound { .. } => "path not found",
            ConfigDiagnostic::EnvExpandError { .. } => "environment expansion error",
        }
    }
}

impl std::error::Error for ConfigDiagnostic {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self {
            ConfigDiagnostic::ParseError(error) => Some(error),
            ConfigDiagnostic::EnvExpandError { error, .. } => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for ConfigDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigDiagnostic::UnknownVariant { variant, expected, source, .. } => {
                write!(
                    f,
                    "unknown variant `{}` at {}, expected {}",
                    variant,
                    source.at_value_str(),
                    expected
                )
            }
            ConfigDiagnostic::PathNotFound { path, .. } => {
                write!(f, "path not found: {}", path)
            }
            ConfigDiagnostic::ParseError(error) => {
                write!(f, "{}", error)
            }
            ConfigDiagnostic::EnvExpandError { error, source, expected } => {
                write!(
                    f,
                    "failed to expand environment variable at {}: {}",
                    source.at_value_str(),
                    error,
                )
            }
        }
    }
}

impl From<kdl::KdlDiagnostic> for ConfigDiagnostic {
    fn from(error: kdl::KdlDiagnostic) -> Self {
        ConfigDiagnostic::ParseError(error)
    }
}

impl Diagnostic for ConfigDiagnostic {
    fn severity(&self) -> Option<miette::Severity> {
        match self {
            ConfigDiagnostic::ParseError(error) => return Some(error.severity),
            _ => {}
        };

        Some(miette::Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            ConfigDiagnostic::UnknownVariant { expected, .. } => {
                return Some(Box::new(format!("expected {}", expected)));
            }
            ConfigDiagnostic::UnknownVariant { expected, source_ref: Some(source_ref), .. } => {
                return Some(Box::new(format!(
                    "define the variant in the referenced {} or select {}",
                    source_ref.at_str(),
                    expected
                )));
            }
            ConfigDiagnostic::PathNotFound { .. } => {
                return Some(Box::new("ensure the path exists".to_string()));
            }
            ConfigDiagnostic::ParseError(error) => return error.help(),
            ConfigDiagnostic::EnvExpandError { expected, .. } => {
                return Some(Box::new(format!("expected {}", expected)));
            }
        };
        None
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        match self {
            ConfigDiagnostic::ParseError(error) => error.labels(),
            Self::UnknownVariant { source: span, source_ref: Some(source_ref), .. } => {
                let here = self.miette_default_label();
                let reference = LabeledSpan::at(
                    source_ref.span().clone(),
                    format!("reference {}", source_ref.at_str()),
                );
                Some(Box::new([here, reference].into_iter()))
            }
            _ => {
                let here = self.miette_default_label();
                Some(Box::new(std::iter::once(here)))
            }
        }
    }
}

/// Used in error messages.
///
/// - expected `a`
/// - expected `a` or `b`
/// - expected one of `a`, `b`, `c`
///
/// The slice of names must not be empty.
#[derive(Debug, Clone)]
pub struct OneOf {
    // names: &'static [&'static str],
    names: Vec<String>,
}

impl From<Vec<String>> for OneOf {
    fn from(names: Vec<String>) -> Self {
        OneOf { names }
    }
}

impl<T> FromIterator<T> for OneOf
where
    T: Display,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        OneOf { names: iter.into_iter().map(|v| v.to_string()).collect() }
    }
}

impl fmt::Display for OneOf {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.names.len() {
            0 => write!(formatter, "there are no variants"), // special case elsewhere
            1 => write!(formatter, "`{}`", self.names[0]),
            2 => write!(formatter, "`{}` or `{}`", self.names[0], self.names[1]),
            _ => {
                formatter.write_str("one of ")?;
                for (i, alt) in self.names.iter().enumerate() {
                    if i > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "`{}`", alt)?;
                }
                Ok(())
            }
        }
    }
}
