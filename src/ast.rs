//! Typed abstract syntax tree for org-mode documents.
//!
//! The AST takes its structural vocabulary from org-mode while staying
//! format-neutral. Every node kind is enumerated as a constructor in a
//! closed-world ADT. Newtype phantoms prevent type confusion between
//! domain identifiers.

use serde::{Deserialize, Serialize};

/// A complete org document: a sequence of top-level blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// Top-level blocks in source order.
    pub blocks: Vec<Block>,
}

/// Block-level structural elements.
///
/// Every node kind in the org structural vocabulary is enumerated as its own
/// constructor. Drawers and planning lines are top-level blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// `Heading level title tags children`
    Heading {
        /// One-based org heading depth.
        level: u8,
        /// Heading text without tags or leading stars.
        title: Title,
        /// Tags attached to the heading.
        tags: Vec<Tag>,
        /// Blocks nested below the heading.
        children: Vec<Self>,
    },
    /// A paragraph containing inline content.
    Paragraph {
        /// Inline content in source order.
        inlines: Vec<Inline>,
    },
    /// `SrcBlock language content`
    SrcBlock {
        /// Language identifier following `#+begin_src`.
        language: String,
        /// Literal block body.
        content: String,
    },
    /// `#+begin_example` … `#+end_example` literal example block.
    ExampleBlock {
        /// Literal example body.
        content: String,
    },
    /// A quote block containing parsed child blocks.
    QuoteBlock {
        /// Blocks nested inside the quote.
        children: Vec<Self>,
    },
    /// An ordered or unordered list.
    List {
        /// Marker style and starting ordinal.
        list_type: ListType,
        /// Items in source order.
        items: Vec<ListItem>,
    },
    /// A table represented as rows of cells.
    Table {
        /// Table rows in source order.
        rows: Vec<Vec<TableCell>>,
    },
    /// An org property drawer.
    PropertyDrawer {
        /// Property keys and verbatim values.
        entries: Vec<(String, String)>,
    },
    /// An org logbook drawer.
    LogbookDrawer {
        /// Parsed log entries.
        entries: Vec<LogEntry>,
    },
    /// An org planning line.
    Planning {
        /// Scheduled, deadline, and closed timestamps.
        entries: Vec<PlanningEntry>,
    },
    /// A comment line.
    Comment {
        /// Comment text without the `# ` prefix.
        text: String,
    },
    /// A `#+NAME: value` keyword line (e.g. `#+title:`, `#+filetags:`).
    ///
    /// `name` is the keyword identifier normalized to lowercase, since org
    /// keywords are case-insensitive; `value` is the verbatim remainder
    /// after the `:` — leading space included — so the line round-trips
    /// byte-for-byte.
    Keyword {
        /// Keyword identifier without `#+` or `:`.
        name: String,
        /// Verbatim value following the colon.
        value: String,
    },
    /// A single empty source line. Blank lines are represented explicitly
    /// so document spacing round-trips faithfully.
    BlankLine,
    /// A horizontal rule represented by five hyphens.
    HorizontalRule,
}

/// Whether a list is ordered (numbered) or unordered (bulleted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListType {
    /// Numbered list; the value is the first item's ordinal, so a list
    /// that starts at `2.` round-trips faithfully.
    Ordered(u64),
    /// Bulleted list.
    Unordered,
}

/// A list item: nested blocks plus checkbox state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    /// Blocks forming the item body.
    pub content: Vec<Block>,
    /// Checkbox state parsed from the item marker.
    pub checkbox: Checkbox,
}

/// Checkbox state for a list item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Checkbox {
    /// The item has no checkbox marker.
    NoCheckbox,
    /// The item has an unchecked `[ ]` marker.
    Unchecked,
    /// The item has a checked `[X]` marker.
    Checked,
}

/// A single table cell containing inline content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    /// Inline content retained verbatim where required for alignment.
    pub inlines: Vec<Inline>,
}

/// One entry on a planning line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanningEntry {
    /// A scheduled timestamp.
    Scheduled(Timestamp),
    /// A deadline timestamp.
    Deadline(Timestamp),
    /// A completion timestamp.
    Closed(Timestamp),
}

/// A logbook entry: a state-change timestamp plus its trailing note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    /// State-change timestamp.
    pub timestamp: Timestamp,
    /// Trailing note text.
    pub note: String,
}

/// Inline formatting elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inline {
    /// Unformatted text.
    Plain(String),
    /// Bold inline content.
    Bold(Vec<Self>),
    /// Italic inline content.
    Italic(Vec<Self>),
    /// Strikethrough markup, rendered as `+text+` in org projection.
    Strikethrough(Vec<Self>),
    /// Inline code delimited by equals signs.
    InlineCode(String),
    /// Verbatim content delimited by tildes.
    Verbatim(String),
    /// A source line break within a paragraph. Generated as `\n`; lets a
    /// multi-line paragraph round-trip without re-wrapping.
    LineBreak,
    /// `Link target description`. `None` description means the link renders
    /// its target as its visible text.
    Link {
        /// Link destination.
        target: String,
        /// Optional visible description.
        description: Option<String>,
    },
}

// ── Newtype phantoms ────────────────────────────────────────────────────

/// Unique identifier for a node, such as the value of an org `:ID:` property.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// A heading title.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Title(pub String);

/// A tag applied to a heading.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tag(pub String);

impl NodeId {
    /// Borrow the inner identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Title {
    /// Borrow the inner title string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Tag {
    /// Borrow the inner tag string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for Title {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Title {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for Tag {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Tag {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An org timestamp in its source-text form, active (`<2026-04-30 Thu>`)
/// or inactive (`[2026-04-30 Thu]`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_newtypes_convert_borrow_and_display() {
        let node_from_str = NodeId::from("node-1");
        let node_from_string = NodeId::from(String::from("node-2"));
        let title_from_str = Title::from("Heading");
        let title_from_string = Title::from(String::from("Other"));
        let tag_from_str = Tag::from("rust");
        let tag_from_string = Tag::from(String::from("org"));

        assert_eq!(node_from_str.as_str(), "node-1");
        assert_eq!(node_from_string.to_string(), "node-2");
        assert_eq!(title_from_str.as_str(), "Heading");
        assert_eq!(title_from_string.to_string(), "Other");
        assert_eq!(tag_from_str.as_str(), "rust");
        assert_eq!(tag_from_string.to_string(), "org");
    }

    #[test]
    fn construct_simple_document() {
        let doc = Document {
            blocks: vec![Block::Heading {
                level: 1,
                title: Title("Hello".into()),
                tags: vec![],
                children: vec![Block::Paragraph {
                    inlines: vec![Inline::Plain("some text".into())],
                }],
            }],
        };
        assert_eq!(doc.blocks.len(), 1);
    }

    #[test]
    fn ast_types_serialize_roundtrip() -> Result<(), serde_json::Error> {
        let doc = Document {
            blocks: vec![
                Block::Heading {
                    level: 1,
                    title: Title("Test".into()),
                    tags: vec![Tag("rust".into())],
                    children: vec![
                        Block::Paragraph {
                            inlines: vec![
                                Inline::Plain("Hello ".into()),
                                Inline::Bold(vec![Inline::Plain("world".into())]),
                                Inline::Plain(". ".into()),
                                Inline::Link {
                                    target: "https://example.com".into(),
                                    description: Some("link".into()),
                                },
                            ],
                        },
                        Block::SrcBlock {
                            language: "rust".into(),
                            content: "fn main() {}".into(),
                        },
                        Block::PropertyDrawer {
                            entries: vec![("ID".into(), "abc-123".into())],
                        },
                    ],
                },
                Block::List {
                    list_type: ListType::Unordered,
                    items: vec![
                        ListItem {
                            content: vec![Block::Paragraph {
                                inlines: vec![Inline::Plain("first".into())],
                            }],
                            checkbox: Checkbox::NoCheckbox,
                        },
                        ListItem {
                            content: vec![Block::Paragraph {
                                inlines: vec![Inline::Plain("second".into())],
                            }],
                            checkbox: Checkbox::Checked,
                        },
                    ],
                },
            ],
        };

        let json = serde_json::to_string(&doc)?;
        let roundtripped: Document = serde_json::from_str(&json)?;
        assert_eq!(doc, roundtripped);
        Ok(())
    }
}
