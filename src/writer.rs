//! Org projection: serialize a [`crate::ast::Document`] back to org-mode text.
//!
//! [`crate::writer::write_document`] inverts [`crate::parser::parse_document`]
//! up to a documented set of normalizations, mirroring org-mode's own
//! interpreter (`org-element-interpret-data`), which projects a normalized
//! form rather than reproducing source bytes.
//!
//! # Normalization rules
//!
//! The projection always applies the rules below; behavior outside this
//! list is a bug:
//!
//! 1. Every block ends with exactly one newline, so a non-empty projection
//!    ends with a newline and blank lines project as empty lines.
//! 2. Non-empty `src` and `example` bodies end with a newline; the writer
//!    appends one to a body that lacks it. An empty language projects with
//!    no trailing space after `#+begin_src`.
//! 3. Keyword lines project a lowercase name with the verbatim value, and
//!    block delimiters project lowercase; org keyword syntax is
//!    case-insensitive.
//! 4. Heading tags project as ` :t1:t2:` with a single space before the
//!    first tag; title text is otherwise verbatim.
//! 5. Ordered list items number consecutively from the list's start
//!    ordinal, so gaps handwritten in the source do not survive.
//! 6. Planning entries project single-space-separated on one line.
//! 7. Table rows project at column 0 with verbatim cell padding; leading
//!    indentation before the first pipe does not survive because the
//!    parser trims table lines.
//! 8. List item content projects its first block inline after the bullet
//!    and any remaining blocks as continuation lines indented by two
//!    spaces. The parser itself only produces single-paragraph items, so
//!    this rule concerns hand-built documents.

use crate::ast::{
    Block, Checkbox, Document, Inline, ListItem, ListType, PlanningEntry, TableCell, Tag, Title,
};

/// Serialize a [`Document`] to org-mode text.
///
/// The projection applies the normalization rules in the module
/// documentation. For documents produced by [`crate::parser::parse_document`]
/// from input already in canonical form, the result is byte-identical to
/// the input.
#[must_use]
pub fn write_document(doc: &Document) -> String {
    let mut out = String::new();
    write_blocks(&mut out, &doc.blocks);
    out
}

/// Project a sequence of blocks.
fn write_blocks(out: &mut String, blocks: &[Block]) {
    for block in blocks {
        write_block(out, block);
    }
}

/// Project one block, including its trailing newline.
fn write_block(out: &mut String, block: &Block) {
    match block {
        Block::Heading {
            level,
            title,
            tags,
            children,
        } => write_heading(out, *level, title, tags, children),
        Block::Paragraph { inlines } => {
            write_inlines(out, inlines);
            out.push('\n');
        }
        Block::SrcBlock { language, content } => {
            out.push_str("#+begin_src");
            if !language.is_empty() {
                out.push(' ');
                out.push_str(language);
            }
            out.push('\n');
            push_body(out, content);
            out.push_str("#+end_src\n");
        }
        Block::ExampleBlock { content } => {
            out.push_str("#+begin_example\n");
            push_body(out, content);
            out.push_str("#+end_example\n");
        }
        Block::QuoteBlock { children } => {
            out.push_str("#+begin_quote\n");
            write_blocks(out, children);
            out.push_str("#+end_quote\n");
        }
        Block::List { list_type, items } => {
            for (index, item) in items.iter().enumerate() {
                write_list_item(out, list_type, index, item);
            }
        }
        Block::Table { rows } => write_table(out, rows),
        Block::PropertyDrawer { entries } => {
            out.push_str(":PROPERTIES:\n");
            for (key, value) in entries {
                out.push(':');
                out.push_str(key);
                out.push(':');
                out.push_str(value);
                out.push('\n');
            }
            out.push_str(":END:\n");
        }
        Block::LogbookDrawer { entries } => {
            out.push_str(":LOGBOOK:\n");
            for entry in entries {
                out.push_str("- ");
                out.push_str(&entry.timestamp.0);
                if !entry.note.is_empty() {
                    out.push(' ');
                    out.push_str(&entry.note);
                }
                out.push('\n');
            }
            out.push_str(":END:\n");
        }
        Block::Planning { entries } => write_planning(out, entries),
        Block::Comment { text } => {
            out.push_str("# ");
            out.push_str(text);
            out.push('\n');
        }
        Block::Keyword { name, value } => {
            out.push_str("#+");
            out.push_str(name);
            out.push(':');
            out.push_str(value);
            out.push('\n');
        }
        Block::BlankLine => out.push('\n'),
        Block::HorizontalRule => out.push_str("-----\n"),
    }
}

/// Project a heading line and its nested children.
fn write_heading(out: &mut String, level: u8, title: &Title, tags: &[Tag], children: &[Block]) {
    for _ in 0..level {
        out.push('*');
    }
    out.push(' ');
    out.push_str(title.as_str());
    if !tags.is_empty() {
        out.push(' ');
        for tag in tags {
            out.push(':');
            out.push_str(tag.as_str());
        }
        out.push(':');
    }
    out.push('\n');
    write_blocks(out, children);
}

/// Project table rows at column 0 with verbatim cell padding.
fn write_table(out: &mut String, rows: &[Vec<TableCell>]) {
    for row in rows {
        out.push('|');
        for cell in row {
            write_inlines(out, &cell.inlines);
            out.push('|');
        }
        out.push('\n');
    }
}

/// Project planning entries single-space-separated on one line.
fn write_planning(out: &mut String, entries: &[PlanningEntry]) {
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        let (keyword, timestamp) = match entry {
            PlanningEntry::Scheduled(ts) => ("SCHEDULED: ", ts),
            PlanningEntry::Deadline(ts) => ("DEADLINE: ", ts),
            PlanningEntry::Closed(ts) => ("CLOSED: ", ts),
        };
        out.push_str(keyword);
        out.push_str(&timestamp.0);
    }
    out.push('\n');
}

/// Project a literal block body, canonicalizing its trailing newline.
fn push_body(out: &mut String, content: &str) {
    if content.is_empty() {
        return;
    }
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
}

/// Project one list item: bullet, checkbox, first block inline, and any
/// remaining blocks as indented continuation lines.
fn write_list_item(out: &mut String, list_type: &ListType, index: usize, item: &ListItem) {
    match list_type {
        ListType::Unordered => out.push_str("- "),
        ListType::Ordered(start) => {
            let ordinal = start.saturating_add(u64::try_from(index).unwrap_or(u64::MAX));
            out.push_str(&ordinal.to_string());
            out.push_str(". ");
        }
    }
    match item.checkbox {
        Checkbox::NoCheckbox => {}
        Checkbox::Unchecked => out.push_str("[ ] "),
        Checkbox::Checked => out.push_str("[X] "),
    }
    match item.content.first() {
        Some(first) => write_block(out, first),
        None => out.push('\n'),
    }
    if item.content.len() > 1 {
        let mut continuation = String::new();
        write_blocks(&mut continuation, item.content.get(1..).unwrap_or(&[]));
        for line in continuation.lines() {
            if !line.is_empty() {
                out.push_str("  ");
                out.push_str(line);
            }
            out.push('\n');
        }
    }
}

/// Project inline content.
fn write_inlines(out: &mut String, inlines: &[Inline]) {
    for inline in inlines {
        write_inline(out, inline);
    }
}

/// Project one inline node.
fn write_inline(out: &mut String, inline: &Inline) {
    match inline {
        Inline::Plain(text) => out.push_str(text),
        Inline::Bold(inner) => {
            out.push('*');
            write_inlines(out, inner);
            out.push('*');
        }
        Inline::Italic(inner) => {
            out.push('/');
            write_inlines(out, inner);
            out.push('/');
        }
        Inline::Strikethrough(inner) => {
            out.push('+');
            write_inlines(out, inner);
            out.push('+');
        }
        Inline::InlineCode(text) => {
            out.push('=');
            out.push_str(text);
            out.push('=');
        }
        Inline::Verbatim(text) => {
            out.push('~');
            out.push_str(text);
            out.push('~');
        }
        Inline::LineBreak => out.push('\n'),
        Inline::Link {
            target,
            description,
        } => {
            out.push_str("[[");
            out.push_str(target);
            if let Some(desc) = description {
                out.push_str("][");
                out.push_str(desc);
            }
            out.push_str("]]");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::write_document;
    use crate::ast::{
        Block, Checkbox, Document, Inline, ListItem, ListType, LogEntry, PlanningEntry, Tag,
        Timestamp, Title,
    };
    use crate::parser::parse_document;

    fn project(input: &str) -> Option<String> {
        parse_document(input).ok().map(|doc| write_document(&doc))
    }

    fn assert_roundtrip(input: &str) {
        assert_eq!(project(input), Some(input.to_owned()), "input: {input:?}");
    }

    fn assert_projection(input: &str, expected: &str) {
        assert_eq!(
            project(input),
            Some(expected.to_owned()),
            "input: {input:?}"
        );
    }

    #[test]
    fn heading_with_tags_roundtrips() {
        assert_roundtrip("* Task :rust:kb:\n");
    }

    #[test]
    fn heading_body_and_spacing_roundtrip() {
        assert_roundtrip("* One\n\nbody\n\n* Two\n");
    }

    #[test]
    fn paragraph_with_line_breaks_roundtrips() {
        assert_roundtrip("first\nsecond\n");
    }

    #[test]
    fn every_inline_marker_roundtrips() {
        assert_roundtrip("a *b* c /d/ e +f+ g =h= i ~j~ k [[l]] [[m][n]]\n");
    }

    #[test]
    fn src_block_roundtrips() {
        assert_roundtrip("#+begin_src rust\nfn main() {}\n#+end_src\n");
    }

    #[test]
    fn src_block_without_language_roundtrips() {
        assert_roundtrip("#+begin_src\nplain\n#+end_src\n");
    }

    #[test]
    fn src_block_body_gains_trailing_newline() {
        let doc = Document {
            blocks: vec![Block::SrcBlock {
                language: "rust".into(),
                content: "fn x() {}".into(),
            }],
        };
        assert_eq!(
            write_document(&doc),
            "#+begin_src rust\nfn x() {}\n#+end_src\n"
        );
    }

    #[test]
    fn example_block_roundtrips() {
        assert_roundtrip("#+begin_example\n$ ls\n#+end_example\n");
    }

    #[test]
    fn quote_block_roundtrips() {
        assert_roundtrip("#+begin_quote\nquoted\n#+end_quote\n");
    }

    #[test]
    fn unordered_list_with_checkboxes_roundtrips() {
        assert_roundtrip("- [ ] todo\n- [X] done\n");
    }

    #[test]
    fn list_continuation_lines_roundtrip() {
        assert_roundtrip("- item\n  continuation\n");
    }

    #[test]
    fn ordered_list_renumbers_consecutively() {
        assert_projection("1. one\n3. three\n", "1. one\n2. three\n");
    }

    #[test]
    fn ordered_list_preserves_start_ordinal() {
        assert_projection("2. two\n2. three\n", "2. two\n3. three\n");
    }

    #[test]
    fn table_rows_roundtrip() {
        assert_roundtrip("| a | b |\n| c | d |\n");
    }

    #[test]
    fn property_drawer_roundtrips() {
        assert_roundtrip(":PROPERTIES:\n:ID: abc-123\n:END:\n");
    }

    #[test]
    fn logbook_entry_without_note_roundtrips() {
        assert_roundtrip(":LOGBOOK:\n- <2026-04-30>\n:END:\n");
    }

    #[test]
    fn planning_entries_collapse_to_single_spaces() {
        assert_projection(
            "SCHEDULED:  <2026-05-01>   DEADLINE: [2026-05-15]\n",
            "SCHEDULED: <2026-05-01> DEADLINE: [2026-05-15]\n",
        );
    }

    #[test]
    fn comment_roundtrips() {
        assert_roundtrip("# remark\n");
    }

    #[test]
    fn keyword_roundtrips() {
        assert_roundtrip("#+title: Example\n");
    }

    #[test]
    fn blank_lines_and_rule_roundtrip() {
        assert_roundtrip("\n\n-----\n");
    }

    #[test]
    fn empty_document_projects_to_empty_string() {
        assert_roundtrip("");
    }

    #[test]
    fn hand_built_document_projects_exactly() {
        let doc = Document {
            blocks: vec![Block::Heading {
                level: 2,
                title: Title("Task".into()),
                tags: vec![Tag("rust".into())],
                children: vec![
                    Block::Planning {
                        entries: vec![PlanningEntry::Closed(Timestamp(
                            "[2026-04-30 Thu 10:00]".into(),
                        ))],
                    },
                    Block::LogbookDrawer {
                        entries: vec![LogEntry {
                            timestamp: Timestamp("[2026-04-30 Thu 10:00]".into()),
                            note: "noted".into(),
                        }],
                    },
                ],
            }],
        };
        assert_eq!(
            write_document(&doc),
            "** Task :rust:\nCLOSED: [2026-04-30 Thu 10:00]\n:LOGBOOK:\n- [2026-04-30 Thu 10:00] noted\n:END:\n"
        );
    }

    #[test]
    fn list_item_extra_blocks_project_indented() {
        let doc = Document {
            blocks: vec![Block::List {
                list_type: ListType::Unordered,
                items: vec![ListItem {
                    content: vec![
                        Block::Paragraph {
                            inlines: vec![Inline::Plain("first".into())],
                        },
                        Block::Paragraph {
                            inlines: vec![Inline::Plain("second".into())],
                        },
                    ],
                    checkbox: Checkbox::NoCheckbox,
                }],
            }],
        };
        assert_eq!(write_document(&doc), "- first\n  second\n");
    }

    #[test]
    fn list_item_without_content_projects_bullet_only() {
        let doc = Document {
            blocks: vec![Block::List {
                list_type: ListType::Unordered,
                items: vec![ListItem {
                    content: vec![],
                    checkbox: Checkbox::Checked,
                }],
            }],
        };
        assert_eq!(write_document(&doc), "- [X] \n");
    }
}
