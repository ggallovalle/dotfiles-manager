use kdl;
use miette::Diagnostic;
use miette::LabeledSpan;
use miette::SourceSpan;
use std::fmt;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq)]
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SettingsDiagnostic {
    UnknownVariant { variant: String, expected: &'static [&'static str], span: SourceSpan },
    ParseError(kdl::KdlDiagnostic),
}

impl SettingsDiagnostic {
    pub fn span(&self) -> Option<&SourceSpan> {
        match self {
            SettingsDiagnostic::UnknownVariant { span, .. } => Some(span),
            _ => None,
        }
    }

    pub fn unknown_variant(
        variant: impl Into<String>,
        expected: &'static [&'static str],
        span: impl Into<SourceSpan>,
    ) -> Self {
        SettingsDiagnostic::UnknownVariant { variant: variant.into(), expected, span: span.into() }
    }
}

impl std::error::Error for SettingsDiagnostic {}

impl fmt::Display for SettingsDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsDiagnostic::UnknownVariant { variant, expected, .. } => {
                if expected.is_empty() {
                    write!(f, "unknown variant `{}`, there are no variants", variant)
                } else {
                    write!(f, "unknown variant `{}`, expected {}", variant, OneOf::new(expected))
                }
            }
            SettingsDiagnostic::ParseError(error) => {
                write!(f, "{}", error)
            }
            _ => Ok(()),
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
            _ => {}
        };
        None
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        match self {
            SettingsDiagnostic::ParseError(error) => return error.labels(),
            _ => {}
        };
        match self.span() {
            None => None,
            Some(span) => {
                Some(Box::new(std::iter::once(LabeledSpan::at(span.clone(), format!("{}", self)))))
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
#[derive(Debug)]
pub struct OneOf {
    // names: &'static [&'static str],
    names: Vec<String>,
}

impl OneOf {
    pub fn new(names: &'static [&'static str]) -> Self {
        OneOf { names: names.iter().map(|s| s.to_string()).collect() }
    }

    pub fn from_vec(names: Vec<String>) -> Self {
        OneOf { names }
    }

    pub fn from_iter<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Display,
    {
        OneOf { names: names.into_iter().map(|v| v.to_string()).collect() }
    }
}

impl fmt::Display for OneOf {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.names.len() {
            0 => panic!(), // special case elsewhere
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
