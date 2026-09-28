//! The typed site manifest.
//!
//! The manifest is produced by an eval-only pass over the Typst entry before
//! layout. It contains no plugin-specific fields: plugins interpret
//! [`PageMeta::data`] as they see fit.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use typst::foundations::Value;

/// A typed, Rust-owned decode of a Typst front-matter value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum FrontMatter {
    /// The absence of a value.
    #[default]
    None,
    /// A boolean.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A string.
    Str(String),
    /// An array.
    Array(Vec<FrontMatter>),
    /// A dictionary / map.
    Dict(BTreeMap<String, FrontMatter>),
    /// A Typst value typost's typed decode does not carry (content, lengths,
    /// colors, ...), holding the Typst type name. Distinct from [`None`], which
    /// means "absent", so plugins can tell "present but undecodable" apart.
    ///
    /// [`None`]: FrontMatter::None
    Unsupported(String),
}

impl FrontMatter {
    /// Decode the supported subset of a Typst value.
    ///
    /// Values outside the supported subset (lengths, colors, content, ...)
    /// decode to [`FrontMatter::None`].
    pub fn from_value(value: &Value) -> Self {
        match value {
            Value::None => Self::None,
            Value::Bool(v) => Self::Bool(*v),
            Value::Int(v) => Self::Int(*v),
            Value::Float(v) => Self::Float(*v),
            Value::Str(v) => Self::Str(v.as_str().to_owned()),
            Value::Array(v) => Self::Array(v.iter().map(Self::from_value).collect()),
            Value::Dict(v) => Self::Dict(
                v.iter()
                    .map(|(k, v)| (k.as_str().to_owned(), Self::from_value(v)))
                    .collect(),
            ),
            other => Self::Unsupported(other.ty().to_string()),
        }
    }

    /// Collect the values that could not be decoded, as `(path, type name)`.
    ///
    /// Paths are dotted for dictionaries and indexed for arrays (e.g.
    /// `hint`, `tags[0]`).
    pub fn unsupported(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        self.collect_unsupported("", &mut out);
        out
    }

    /// Recursive half of [`unsupported`](Self::unsupported).
    fn collect_unsupported(&self, path: &str, out: &mut Vec<(String, String)>) {
        match self {
            Self::Unsupported(ty) => out.push((path.to_owned(), ty.clone())),
            Self::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    item.collect_unsupported(&format!("{path}[{index}]"), out);
                }
            }
            Self::Dict(map) => {
                for (key, value) in map {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    value.collect_unsupported(&child, out);
                }
            }
            _ => {}
        }
    }

    /// Look up a key in a dictionary, if this is one.
    pub fn get(&self, key: &str) -> Option<&FrontMatter> {
        match self {
            Self::Dict(map) => map.get(key),
            _ => None,
        }
    }

    /// Borrow the value as a string, if it is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(v) => Some(v),
            _ => None,
        }
    }

    /// Borrow the value as a boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Borrow the value as an array, if it is one.
    pub fn as_array(&self) -> Option<&[FrontMatter]> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }
}

/// One page of the site.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageMeta {
    /// The output path, relative to the site root (e.g. `hello/index.html`).
    pub route: String,
    /// The Typst source that produced this page, if known.
    pub src: Option<PathBuf>,
    /// The decoded front matter from the page's `typost` binding.
    pub data: FrontMatter,
}

/// The site structure, known before rendering.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteManifest {
    /// The site title, if declared.
    pub title: Option<String>,
    /// All pages, in declaration order.
    pub pages: Vec<PageMeta>,
    /// Generic, plugin-defined entries collected from `typost` metadata
    /// elements. Core does not interpret them; plugins read the ones they
    /// declare via [`MetadataEntry::kind`].
    #[serde(default)]
    pub entries: Vec<MetadataEntry>,
}

/// A plugin-defined metadata entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataEntry {
    /// The `typost.kind` of the entry, e.g. `"tags"`.
    pub kind: String,
    /// The route of the enclosing document, when the entry was emitted inside
    /// one.
    pub route: Option<String>,
    /// The entry's `typost` dictionary (including `kind`).
    pub data: FrontMatter,
}

#[cfg(test)]
mod tests {
    use super::FrontMatter;
    use std::collections::BTreeMap;

    #[test]
    fn collects_undecodable_values_with_paths() {
        let mut dict = BTreeMap::new();
        dict.insert("ok".to_owned(), FrontMatter::Str("x".to_owned()));
        dict.insert(
            "rich".to_owned(),
            FrontMatter::Unsupported("content".to_owned()),
        );
        dict.insert(
            "tags".to_owned(),
            FrontMatter::Array(vec![
                FrontMatter::Str("a".to_owned()),
                FrontMatter::Unsupported("length".to_owned()),
            ]),
        );
        dict.insert(
            "nested".to_owned(),
            FrontMatter::Dict(BTreeMap::from([(
                "deep".to_owned(),
                FrontMatter::Unsupported("color".to_owned()),
            )])),
        );

        let mut found = FrontMatter::Dict(dict).unsupported();
        found.sort();
        assert_eq!(
            found,
            vec![
                ("nested.deep".to_owned(), "color".to_owned()),
                ("rich".to_owned(), "content".to_owned()),
                ("tags[1]".to_owned(), "length".to_owned()),
            ]
        );
    }
}
