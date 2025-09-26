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
pub struct SettingsError {
    pub input: Arc<String>,
    pub diagnostics: Vec<SettingsDiagnostic>,
    named_source: Option<miette::NamedSource<Arc<String>>>, // store it here to support Diagnostic::source_code
}

impl SettingsError {
    pub fn from_str(input: impl Into<Arc<String>>, diagnostics: Vec<SettingsDiagnostic>) -> Self {
        let input = input.into();
        SettingsError { input, diagnostics, named_source: None }
    }

    pub fn from_file(
        name: &PathBuf,
        input: Arc<String>,
        diagnostics: Vec<SettingsDiagnostic>,
    ) -> Self {
        let named_source = Some(
            miette::NamedSource::new(name.to_string_lossy(), input.clone()).with_language("kdl"),
        );
        SettingsError { input, diagnostics, named_source }
    }
}

impl SettingsError {
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

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.named_source {
            write!(f, "settings error in {}", name.name())
        } else {
            write!(f, "settings error")
        }
    }
}

impl std::error::Error for SettingsError {}

impl Diagnostic for SettingsError {
    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        if let Some(name) = &self.named_source { Some(name) } else { Some(&*self.input) }
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        Some(Box::new(self.diagnostics.iter().map(|d| d as &dyn Diagnostic)))
    }
}

#[derive(Debug, Clone)]
pub enum SettingsDiagnostic {
    UnknownVariant { variant: String, expected: OneOf, span: SourceSpan },
    PathNotFound { path: String, span: SourceSpan },
    ParseError(kdl::KdlDiagnostic),
}

impl SettingsDiagnostic {
    pub fn unknown_variant(
        span: impl Into<SourceSpan>,
        variant: impl Into<String>,
        expected: impl Into<OneOf>,
    ) -> Self {
        SettingsDiagnostic::UnknownVariant {
            variant: variant.into(),
            expected: expected.into(),
            span: span.into(),
        }
    }

    pub fn path_not_found(span: impl Into<SourceSpan>, path: impl Into<String>) -> Self {
        SettingsDiagnostic::PathNotFound { path: path.into(), span: span.into() }
    }

    pub fn span(&self) -> SourceSpan {
        match self {
            SettingsDiagnostic::UnknownVariant { span, .. } => span.clone(),
            SettingsDiagnostic::ParseError(error) => error.span.clone(),
            SettingsDiagnostic::PathNotFound { span, .. } => span.clone(),
        }
    }

    fn miette_default_label(&self) -> LabeledSpan {
        LabeledSpan::at(self.span(), "here")
    }

    fn kind(&self) -> &str {
        match self {
            SettingsDiagnostic::UnknownVariant { .. } => "unknown variant",
            SettingsDiagnostic::ParseError(_) => "parse error",
            SettingsDiagnostic::PathNotFound { .. } => "path not found",
        }
    }
}

impl std::error::Error for SettingsDiagnostic {}

impl fmt::Display for SettingsDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsDiagnostic::UnknownVariant { variant, expected, .. } => {
                write!(f, "unknown variant `{}`, expected {}", variant, expected)
            }
            SettingsDiagnostic::PathNotFound { path, .. } => {
                write!(f, "path not found: {}", path)
            }
            SettingsDiagnostic::ParseError(error) => {
                write!(f, "{}", error)
            }
        }
    }
}

impl From<kdl::KdlDiagnostic> for SettingsDiagnostic {
    fn from(error: kdl::KdlDiagnostic) -> Self {
        SettingsDiagnostic::ParseError(error)
    }
}

impl Diagnostic for SettingsDiagnostic {
    fn severity(&self) -> Option<miette::Severity> {
        match self {
            SettingsDiagnostic::ParseError(error) => return Some(error.severity),
            _ => {}
        };

        Some(miette::Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            SettingsDiagnostic::ParseError(error) => return error.help(),
            SettingsDiagnostic::UnknownVariant { expected, .. } => {
                return Some(Box::new(format!("expected {}", expected)));
            }
            SettingsDiagnostic::PathNotFound { .. } => {
                return Some(Box::new("ensure the path exists".to_string()));
            }
        };
        None
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        match self {
            SettingsDiagnostic::ParseError(error) => error.labels(),
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

impl From<&'static [&'static str]> for OneOf {
    fn from(names: &'static [&'static str]) -> Self {
        OneOf { names: names.iter().map(|s| s.to_string()).collect() }
    }
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

impl OneOf {
    pub fn new(names: &'static [&'static str]) -> Self {
        OneOf::from(names)
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
