//! Parser specification tests, ported from an earlier Haskell implementation.
//!
//! Each test pins one input string to its expected `Document`, and the
//! suite keeps the case-for-case shape of the original specification.

#![allow(clippy::needless_pass_by_value, clippy::missing_const_for_fn)]

use tftio_org::ast::{
    Block, Checkbox, Document, Inline, ListItem, ListType, LogEntry, PlanningEntry, TableCell, Tag,
    Timestamp, Title,
};
use tftio_org::parser::parse_document;

fn expect(input: &str, want: Document) {
    assert_eq!(parse_document(input).ok(), Some(want));
}

fn doc(blocks: Vec<Block>) -> Document {
    Document { blocks }
}

fn heading(level: u8, title: &str, tags: Vec<Tag>) -> Block {
    Block::Heading {
        level,
        title: Title(title.into()),
        tags,
        children: vec![],
    }
}

fn para(inlines: Vec<Inline>) -> Block {
    Block::Paragraph { inlines }
}

fn plain(s: &str) -> Inline {
    Inline::Plain(s.into())
}

#[test]
fn single_level_one_heading() {
    expect("* Hello\n", doc(vec![heading(1, "Hello", vec![])]));
}

#[test]
fn deep_heading_level() {
    expect("**** Deep\n", doc(vec![heading(4, "Deep", vec![])]));
}

#[test]
fn heading_followed_by_paragraph_nests_the_paragraph() {
    expect(
        "* Hello\nSome text\n",
        doc(vec![Block::Heading {
            level: 1,
            title: Title("Hello".into()),
            tags: vec![],
            children: vec![para(vec![plain("Some text")])],
        }]),
    );
}

#[test]
fn blank_lines_and_body_belong_to_the_preceding_heading() {
    expect(
        "* One\n\nbody\n\n* Two\n",
        doc(vec![
            Block::Heading {
                level: 1,
                title: Title("One".into()),
                tags: vec![],
                children: vec![
                    Block::BlankLine,
                    para(vec![plain("body")]),
                    Block::BlankLine,
                ],
            },
            heading(1, "Two", vec![]),
        ]),
    );
}

#[test]
fn leading_and_trailing_blank_lines_represented() {
    // Blank lines are explicit `BlankLine` blocks for faithful spacing.
    expect(
        "\n\n* Only\n\n\n",
        doc(vec![
            Block::BlankLine,
            Block::BlankLine,
            Block::Heading {
                level: 1,
                title: Title("Only".into()),
                tags: vec![],
                children: vec![Block::BlankLine, Block::BlankLine],
            },
        ]),
    );
}

#[test]
fn empty_input_parses_to_empty_document() {
    expect("", doc(vec![]));
}

#[test]
fn stars_without_trailing_space_are_paragraph_text() {
    expect("*Bad\n", doc(vec![para(vec![plain("*Bad")])]));
}

#[test]
fn bold_inline() {
    expect(
        "*bold*\n",
        doc(vec![para(vec![Inline::Bold(vec![plain("bold")])])]),
    );
}

#[test]
fn italic_inline() {
    expect(
        "/it/\n",
        doc(vec![para(vec![Inline::Italic(vec![plain("it")])])]),
    );
}

#[test]
fn inline_code() {
    expect(
        "=code=\n",
        doc(vec![para(vec![Inline::InlineCode("code".into())])]),
    );
}

#[test]
fn verbatim() {
    expect(
        "~verb~\n",
        doc(vec![para(vec![Inline::Verbatim("verb".into())])]),
    );
}

#[test]
fn link_without_description() {
    expect(
        "[[id:abc]]\n",
        doc(vec![para(vec![Inline::Link {
            target: "id:abc".into(),
            description: None,
        }])]),
    );
}

#[test]
fn link_with_description() {
    expect(
        "[[id:abc][label]]\n",
        doc(vec![para(vec![Inline::Link {
            target: "id:abc".into(),
            description: Some("label".into()),
        }])]),
    );
}

#[test]
fn mixed_plain_and_formatted_text_merges_plain_runs() {
    expect(
        "before *b* after\n",
        doc(vec![para(vec![
            plain("before "),
            Inline::Bold(vec![plain("b")]),
            plain(" after"),
        ])]),
    );
}

#[test]
fn italic_nested_inside_bold() {
    expect(
        "*/foo/*\n",
        doc(vec![para(vec![Inline::Bold(vec![Inline::Italic(vec![
            plain("foo"),
        ])])])]),
    );
}

#[test]
fn stray_asterisk_falls_back_to_plain() {
    expect("a * b\n", doc(vec![para(vec![plain("a * b")])]));
}

#[test]
fn property_drawer_with_one_entry() {
    expect(
        ":PROPERTIES:\n:ID: abc-123\n:END:\n",
        doc(vec![Block::PropertyDrawer {
            // Values verbatim, including the space after the key colon.
            entries: vec![("ID".into(), " abc-123".into())],
        }]),
    );
}

#[test]
fn property_drawer_with_multiple_entries() {
    expect(
        ":PROPERTIES:\n:ID: x\n:CATEGORY: kb\n:END:\n",
        doc(vec![Block::PropertyDrawer {
            entries: vec![
                ("ID".into(), " x".into()),
                ("CATEGORY".into(), " kb".into()),
            ],
        }]),
    );
}

#[test]
fn empty_property_drawer() {
    expect(
        ":PROPERTIES:\n:END:\n",
        doc(vec![Block::PropertyDrawer { entries: vec![] }]),
    );
}

#[test]
fn src_block_with_language_and_single_line_body() {
    expect(
        "#+begin_src sh\necho hi\n#+end_src\n",
        doc(vec![Block::SrcBlock {
            language: "sh".into(),
            content: "echo hi\n".into(),
        }]),
    );
}

#[test]
fn src_block_with_multi_line_body() {
    expect(
        "#+begin_src haskell\nmain :: IO ()\nmain = pure ()\n#+end_src\n",
        doc(vec![Block::SrcBlock {
            language: "haskell".into(),
            content: "main :: IO ()\nmain = pure ()\n".into(),
        }]),
    );
}

#[test]
fn src_block_without_language() {
    expect(
        "#+begin_src\nplain\n#+end_src\n",
        doc(vec![Block::SrcBlock {
            language: String::new(),
            content: "plain\n".into(),
        }]),
    );
}

#[test]
fn src_block_surrounded_by_paragraphs() {
    expect(
        "before\n\n#+begin_src sh\nhi\n#+end_src\n\nafter\n",
        doc(vec![
            para(vec![plain("before")]),
            Block::BlankLine,
            Block::SrcBlock {
                language: "sh".into(),
                content: "hi\n".into(),
            },
            Block::BlankLine,
            para(vec![plain("after")]),
        ]),
    );
}

#[test]
fn single_scheduled_planning_entry() {
    expect(
        "SCHEDULED: <2026-05-01>\n",
        doc(vec![Block::Planning {
            entries: vec![PlanningEntry::Scheduled(Timestamp("<2026-05-01>".into()))],
        }]),
    );
}

#[test]
fn scheduled_plus_deadline_on_one_line() {
    expect(
        "SCHEDULED: <2026-05-01> DEADLINE: <2026-05-15>\n",
        doc(vec![Block::Planning {
            entries: vec![
                PlanningEntry::Scheduled(Timestamp("<2026-05-01>".into())),
                PlanningEntry::Deadline(Timestamp("<2026-05-15>".into())),
            ],
        }]),
    );
}

#[test]
fn closed_planning_entry() {
    expect(
        "CLOSED: <2026-04-30>\n",
        doc(vec![Block::Planning {
            entries: vec![PlanningEntry::Closed(Timestamp("<2026-04-30>".into()))],
        }]),
    );
}

#[test]
fn logbook_drawer_with_one_entry() {
    expect(
        ":LOGBOOK:\n- <2026-04-30 Thu> noted\n:END:\n",
        doc(vec![Block::LogbookDrawer {
            entries: vec![LogEntry {
                timestamp: Timestamp("<2026-04-30 Thu>".into()),
                note: "noted".into(),
            }],
        }]),
    );
}

#[test]
fn logbook_drawer_with_no_note_entry() {
    expect(
        ":LOGBOOK:\n- <2026-04-30>\n:END:\n",
        doc(vec![Block::LogbookDrawer {
            entries: vec![LogEntry {
                timestamp: Timestamp("<2026-04-30>".into()),
                note: String::new(),
            }],
        }]),
    );
}

#[test]
fn empty_logbook_drawer() {
    expect(
        ":LOGBOOK:\n:END:\n",
        doc(vec![Block::LogbookDrawer { entries: vec![] }]),
    );
}

#[test]
fn unordered_list_one_item_no_checkbox() {
    expect(
        "- alpha\n",
        doc(vec![Block::List {
            list_type: ListType::Unordered,
            items: vec![ListItem {
                content: vec![para(vec![plain("alpha")])],
                checkbox: Checkbox::NoCheckbox,
            }],
        }]),
    );
}

#[test]
fn ordered_list_two_items() {
    expect(
        "1. one\n1. two\n",
        doc(vec![Block::List {
            list_type: ListType::Ordered(1),
            items: vec![
                ListItem {
                    content: vec![para(vec![plain("one")])],
                    checkbox: Checkbox::NoCheckbox,
                },
                ListItem {
                    content: vec![para(vec![plain("two")])],
                    checkbox: Checkbox::NoCheckbox,
                },
            ],
        }]),
    );
}

#[test]
fn checkbox_states() {
    expect(
        "- [ ] todo\n- [X] done\n",
        doc(vec![Block::List {
            list_type: ListType::Unordered,
            items: vec![
                ListItem {
                    content: vec![para(vec![plain("todo")])],
                    checkbox: Checkbox::Unchecked,
                },
                ListItem {
                    content: vec![para(vec![plain("done")])],
                    checkbox: Checkbox::Checked,
                },
            ],
        }]),
    );
}

#[test]
fn list_with_empty_item_content() {
    expect(
        "- \n",
        doc(vec![Block::List {
            list_type: ListType::Unordered,
            items: vec![ListItem {
                content: vec![para(vec![])],
                checkbox: Checkbox::NoCheckbox,
            }],
        }]),
    );
}

#[test]
fn heading_with_single_tag() {
    expect(
        "* Title :foo:\n",
        doc(vec![heading(1, "Title", vec![Tag("foo".into())])]),
    );
}

#[test]
fn heading_with_multiple_tags() {
    expect(
        "* Title :foo:bar:\n",
        doc(vec![heading(
            1,
            "Title",
            vec![Tag("foo".into()), Tag("bar".into())],
        )]),
    );
}

#[test]
fn heading_without_tags_but_trailing_colon() {
    expect("* Title:\n", doc(vec![heading(1, "Title:", vec![])]));
}

#[test]
fn heading_body_is_nested_while_the_next_heading_is_a_sibling() {
    expect(
        "* Parent\nbody\n** Child\n",
        doc(vec![
            Block::Heading {
                level: 1,
                title: Title("Parent".into()),
                tags: vec![],
                children: vec![para(vec![plain("body")])],
            },
            heading(2, "Child", vec![]),
        ]),
    );
}

#[test]
fn comment_line() {
    expect(
        "# remark\n",
        doc(vec![Block::Comment {
            text: "remark".into(),
        }]),
    );
}

#[test]
fn horizontal_rule() {
    expect("-----\n", doc(vec![Block::HorizontalRule]));
}

#[test]
fn quote_block_with_paragraph() {
    expect(
        "#+begin_quote\nquoted\n#+end_quote\n",
        doc(vec![Block::QuoteBlock {
            children: vec![para(vec![plain("quoted")])],
        }]),
    );
}

#[test]
fn empty_quote_block() {
    expect(
        "#+begin_quote\n#+end_quote\n",
        doc(vec![Block::QuoteBlock { children: vec![] }]),
    );
}

#[test]
fn table_single_row_two_cells() {
    expect(
        "| a | b |\n",
        doc(vec![Block::Table {
            // Cells are verbatim, including their padding spaces.
            rows: vec![vec![
                TableCell {
                    inlines: vec![plain(" a ")],
                },
                TableCell {
                    inlines: vec![plain(" b ")],
                },
            ]],
        }]),
    );
}

#[test]
fn table_multiple_rows() {
    expect(
        "| a | b |\n| c | d |\n",
        doc(vec![Block::Table {
            rows: vec![
                vec![
                    TableCell {
                        inlines: vec![plain(" a ")],
                    },
                    TableCell {
                        inlines: vec![plain(" b ")],
                    },
                ],
                vec![
                    TableCell {
                        inlines: vec![plain(" c ")],
                    },
                    TableCell {
                        inlines: vec![plain(" d ")],
                    },
                ],
            ],
        }]),
    );
}

#[test]
fn table_with_empty_cell() {
    expect(
        "| a |  | c |\n",
        doc(vec![Block::Table {
            rows: vec![vec![
                TableCell {
                    inlines: vec![plain(" a ")],
                },
                TableCell {
                    inlines: vec![plain("  ")],
                },
                TableCell {
                    inlines: vec![plain(" c ")],
                },
            ]],
        }]),
    );
}
