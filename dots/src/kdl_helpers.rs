use crate::settings_error::SettingsDiagnostic;
use kdl::{KdlDiagnostic, KdlEntry};
use miette::SourceSpan;

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

pub fn prop<'a>(node: &'a kdl::KdlNode, key: &str) -> Result<&'a KdlEntry, KdlDiagnostic> {
    let entry = node.entry(key).ok_or_else(|| {
        diag!(
            node.span(),
            message = format!("node '{}' requires property '{}'", node.name().value(), key)
        )
    })?;
    Ok(entry)
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

pub fn arg(node: &kdl::KdlNode, index: usize) -> Result<&KdlEntry, KdlDiagnostic> {
    let value = node.entry(index).ok_or_else(|| {
        diag!(
            node.span(),
            message = format!("node '{}' requires argument '{}'", node.name().value(), index + 1)
        )
    })?;
    Ok(value)
}

pub fn args(node: &kdl::KdlNode) -> Result<impl Iterator<Item = &KdlEntry>, KdlDiagnostic> {
    let entries = node.entries().iter().filter(|e| e.name().is_none());
    if entries.clone().count() == 0 {
        return Err(diag!(
            node.span(),
            message = format!("node '{}' requires at least one argument", node.name().value())
        ));
    }
    Ok(entries)
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

#[macro_export]
macro_rules! impl_from_kdl_entry_for_enum {
    ($ty:ty) => {
        impl $ty {
            fn from_kdl_entry(entry: &kdl::KdlEntry) -> Result<Self, SettingsDiagnostic> {
                let value = String::from_kdl_entry(entry)?;
                value.parse::<$ty>().map_err(|_| {
                    SettingsDiagnostic::unknown_variant(
                        entry,
                        value,
                        OneOf::from_iter(<$ty>::VARIANTS),
                    )
                })
            }
        }
    };
}

pub trait KdlDocumentExt {
    fn get_span(&self) -> miette::SourceSpan;

    fn get_children<'a>(&'a self) -> impl Iterator<Item = &'a kdl::KdlNode>;

    fn get_children_named<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a kdl::KdlNode> {
        self.get_children().filter(move |n| n.name().value() == name)
    }

    fn get_node(&self, name: &str) -> Option<&kdl::KdlNode> {
        for node in self.get_children() {
            if node.name().value() == name {
                return Some(node);
            }
        }
        None
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
            return Err(diag!(self.get_span(), message = format!("node '{}' is required", name)));
        }
        Ok(found.unwrap())
    }
}

impl KdlDocumentExt for kdl::KdlDocument {
    fn get_span(&self) -> miette::SourceSpan {
        self.span()
    }

    fn get_node(&self, name: &str) -> Option<&kdl::KdlNode> {
        self.get(name)
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

#[derive(Debug, Clone)]
pub enum KdlItemRef {
    Document(SourceSpan),
    Node(SourceSpan),
    EntryArg { entry: SourceSpan, value: SourceSpan },
    EntryProp { entry: SourceSpan, key: SourceSpan, value: SourceSpan },
    Unknown(SourceSpan),
}

impl KdlItemRef {
    pub fn at_str(&self) -> &str {
        match self {
            KdlItemRef::Document(_) => "document",
            KdlItemRef::Node(_) => "node",
            KdlItemRef::EntryArg { .. } => "argument",
            KdlItemRef::EntryProp { .. } => "property",
            KdlItemRef::Unknown(_) => "unknown",
        }
    }

    pub fn at_value_str(&self) -> &str {
        match self {
            KdlItemRef::Document(_) => "document",
            KdlItemRef::Node(_) => "node",
            KdlItemRef::EntryArg { .. } => "argument value",
            KdlItemRef::EntryProp { .. } => "property value",
            KdlItemRef::Unknown(_) => "unknown",
        }
    }

    pub fn span(&self) -> SourceSpan {
        match self {
            KdlItemRef::Document(span) => *span,
            KdlItemRef::Node(span) => *span,
            KdlItemRef::EntryArg { entry, .. } => *entry,
            KdlItemRef::EntryProp { entry, .. } => *entry,
            KdlItemRef::Unknown(span) => *span,
        }
    }

    pub fn span_value(&self) -> SourceSpan {
        match self {
            KdlItemRef::Document(span) => *span,
            KdlItemRef::Node(span) => *span,
            KdlItemRef::EntryArg { value, .. } => *value,
            KdlItemRef::EntryProp { value, .. } => *value,
            KdlItemRef::Unknown(span) => *span,
        }
    }

    pub fn span_key(&self) -> SourceSpan {
        match self {
            KdlItemRef::Document(span) => *span,
            KdlItemRef::Node(span) => *span,
            KdlItemRef::EntryArg { entry, .. } => *entry,
            KdlItemRef::EntryProp { key, .. } => *key,
            KdlItemRef::Unknown(span) => *span,
        }
    }
}

impl From<&kdl::KdlDocument> for KdlItemRef {
    fn from(value: &kdl::KdlDocument) -> Self {
        KdlItemRef::Document(value.span())
    }
}

impl From<&kdl::KdlNode> for KdlItemRef {
    fn from(value: &kdl::KdlNode) -> Self {
        KdlItemRef::Node(value.span())
    }
}

impl From<&kdl::KdlEntry> for KdlItemRef {
    fn from(value: &kdl::KdlEntry) -> Self {
        match (value.name(), value.value()) {
            (Some(name), v) => {
                let name_span = name.span();
                let entry_span = value.span();
                // = and openning " if string
                // let padding = if v.is_string() { 2 } else { 1 };
                let padding = 1;
                let offset = entry_span.offset() + name_span.len() + padding;
                let len = entry_span.len() - name_span.len() - padding;
                let value_span = SourceSpan::new(offset.into(), len);

                KdlItemRef::EntryProp { entry: entry_span, key: name_span, value: value_span }
            }
            (None, v) => KdlItemRef::EntryArg { entry: value.span(), value: value.span() },
        }
    }
}

// impl From<SourceSpan> for KdlItemRef {
//     fn from(value: SourceSpan) -> Self {
//         KdlItemRef::Unknown(value)
//     }
// }
