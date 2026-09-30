//! Conformance cases ported from org-mode's test suite.
//!
//! Each case cites the upstream `ert-deftest` it ports (file
//! `testing/lisp/test-org-element.el` in the org-mode repository) and is
//! one of two kinds:
//!
//! - `Conforms`: the parser's reading matches org-mode's specified
//!   semantics exactly, and the round-trip contracts hold.
//! - `Diverges`: our reading differs from org's spec today. The entry pins
//!   current behavior as a characterization and carries a divergence id
//!   listed in `conformance-DIVERGENCES.md`.
//!
//! The accounting test asserts the corpus divergence ids and the ledger
//! ids are the same set, so a divergence cannot appear or disappear
//! without both changing together. Nothing here is skipped or ignored.

use std::collections::BTreeSet;

use tftio_org::ast::{Block, Document, Inline, ListItem, ListType, TableCell, Tag, Title};
use tftio_org::parser::parse_document;
use tftio_org::writer::write_document;

enum Expectation {
    /// The org-specified document.
    Conforms(Document),
    /// Current behavior, pinned under a ledger id.
    Diverges { id: &'static str, doc: Document },
}

struct Case {
    /// Upstream citation: the `ert-deftest` this case ports.
    origin: &'static str,
    input: &'static str,
    expectation: Expectation,
}

fn cases() -> Vec<Case> {
    let mut all = headline_cases();
    all.extend(block_cases());
    all.extend(list_cases());
    all
}

fn headline_cases() -> Vec<Case> {
    vec![
        // ── test-org-element/headline-tags ─────────────────────────────────
        Case {
            origin: "headline-tags",
            input: "* Headline",
            expectation: Expectation::Conforms(heading(1, "Headline", &[])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline:notatag:",
            expectation: Expectation::Conforms(heading(1, "Headline:notatag:", &[])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :tag:",
            expectation: Expectation::Conforms(heading(1, "Headline", &["tag"])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :tag:tag2:",
            expectation: Expectation::Conforms(heading(1, "Headline", &["tag", "tag2"])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :tag_valid:",
            expectation: Expectation::Conforms(heading(1, "Headline", &["tag_valid"])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :tag_valid#1%%:",
            expectation: Expectation::Conforms(heading(1, "Headline", &["tag_valid#1%%"])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :tag^:",
            expectation: Expectation::Conforms(heading(1, "Headline :tag^:", &[])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :tag::",
            expectation: Expectation::Conforms(heading(1, "Headline", &["tag", ""])),
        },
        Case {
            origin: "headline-tags",
            input: "* Headline :notatag: :tag:",
            expectation: Expectation::Conforms(heading(1, "Headline :notatag:", &["tag"])),
        },
        Case {
            origin: "headline-tags",
            input: "* :notatag: Headline",
            expectation: Expectation::Conforms(heading(1, ":notatag: Headline", &[])),
        },
        Case {
            origin: "headline-tags",
            input: "* :tag:",
            expectation: Expectation::Conforms(heading(1, "", &["tag"])),
        },
        // ── headline semantics org splits and we keep raw ──────────────────
        Case {
            origin: "headline-todo-keyword",
            input: "* TODO Headline",
            expectation: diverges("D-TODO-KEYWORD", heading(1, "TODO Headline", &[])),
        },
        Case {
            origin: "headline-comment-keyword",
            input: "* COMMENT title",
            expectation: diverges("D-COMMENT-KEYWORD", heading(1, "COMMENT title", &[])),
        },
        Case {
            origin: "interpret-data (priority cookies)",
            input: "* [#B] Headline",
            expectation: diverges("D-PRIORITY-COOKIE", heading(1, "[#B] Headline", &[])),
        },
    ]
}

fn block_cases() -> Vec<Case> {
    vec![
        // ── test-org-element/example-block-parser ──────────────────────────
        Case {
            origin: "example-block-parser",
            input: "#+BEGIN_EXAMPLE\nText\n#+END_EXAMPLE",
            expectation: Expectation::Conforms(example("Text\n")),
        },
        Case {
            origin: "interpret-data (keywords)",
            input: "#+keyword: value",
            expectation: Expectation::Conforms(keyword("keyword", " value")),
        },
        // ── block bodies and switches ──────────────────────────────────────
        Case {
            origin: "example-block-parser (un-escaping)",
            input: "#+BEGIN_EXAMPLE\n,* Headline\n ,#+keyword:\nText\n#+END_EXAMPLE",
            expectation: diverges(
                "D-EXAMPLE-COMMA-ESCAPE",
                example(",* Headline\n ,#+keyword:\nText\n"),
            ),
        },
        Case {
            origin: "example-block-parser (incomplete block)",
            input: "#+BEGIN_EXAMPLE",
            expectation: diverges("D-UNTERMINATED-EXAMPLE", example("")),
        },
        Case {
            origin: "block-switches",
            input: "#+BEGIN_SRC emacs-lisp -n -r\n(+ 1 1)\n#+END_SRC",
            expectation: diverges(
                "D-SRC-SWITCHES",
                Document {
                    blocks: vec![Block::SrcBlock {
                        language: "emacs-lisp -n -r".into(),
                        content: "(+ 1 1)\n".into(),
                    }],
                },
            ),
        },
        // ── structural elements not modeled ────────────────────────────────
        Case {
            origin: "drawer parser",
            input: ":TEST:\nTest\n:END:",
            expectation: diverges(
                "D-GENERAL-DRAWERS",
                para_lines(&[":TEST:", "Test", ":END:"]),
            ),
        },
        Case {
            origin: "interpret-data (clock)",
            input: "CLOCK: [2012-01-01 Sun 00:01]",
            expectation: diverges(
                "D-CLOCK-LINE",
                para_lines(&["CLOCK: [2012-01-01 Sun 00:01]"]),
            ),
        },
        Case {
            origin: "interpret-data (fixed width)",
            input: ": Test",
            expectation: diverges("D-FIXED-WIDTH", para_lines(&[": Test"])),
        },
        Case {
            origin: "interpret-data (horizontal rule)",
            input: "-------",
            expectation: diverges("D-RULE-LENGTH", para_lines(&["-------"])),
        },
        Case {
            origin: "interpret-data (keywords)",
            input: "#+KEYWORD: value",
            expectation: diverges("D-KEYWORD-CASE", keyword("keyword", " value")),
        },
    ]
}

fn list_cases() -> Vec<Case> {
    vec![
        // ── list grammar ───────────────────────────────────────────────────
        Case {
            origin: "interpret-data (lists)",
            input: "-",
            expectation: diverges("D-BARE-BULLET", para_lines(&["-"])),
        },
        Case {
            origin: "interpret-data (lists)",
            input: "1) Item",
            expectation: diverges("D-ORDERED-PAREN", para_lines(&["1) Item"])),
        },
        Case {
            origin: "interpret-data (lists)",
            input: "1. [@5] Item",
            expectation: diverges(
                "D-COUNTER-COOKIE",
                list(ListType::Ordered(1), &["[@5] Item"]),
            ),
        },
        Case {
            origin: "interpret-data (lists)",
            input: "-\n  | a | b |",
            expectation: diverges(
                "D-ITEM-BLOCKS",
                Document {
                    blocks: vec![
                        para_block(&["-"]),
                        Block::Table {
                            rows: vec![vec![cell(" a "), cell(" b ")]],
                        },
                    ],
                },
            ),
        },
        Case {
            origin: "interpret-data (lists)",
            input: "- tag :: desc",
            expectation: diverges(
                "D-DESCRIPTION-LIST",
                list(ListType::Unordered, &["tag :: desc"]),
            ),
        },
    ]
}

const fn diverges(id: &'static str, doc: Document) -> Expectation {
    Expectation::Diverges { id, doc }
}

fn heading(level: u8, title: &str, tags: &[&str]) -> Document {
    Document {
        blocks: vec![Block::Heading {
            level,
            title: Title(title.into()),
            tags: tags.iter().map(|tag| Tag((*tag).into())).collect(),
            children: vec![],
        }],
    }
}

fn para_block(lines: &[&str]) -> Block {
    let mut inlines = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            inlines.push(Inline::LineBreak);
        }
        inlines.push(Inline::Plain((*line).into()));
    }
    Block::Paragraph { inlines }
}

fn para_lines(lines: &[&str]) -> Document {
    Document {
        blocks: vec![para_block(lines)],
    }
}

fn example(content: &str) -> Document {
    Document {
        blocks: vec![Block::ExampleBlock {
            content: content.into(),
        }],
    }
}

fn keyword(name: &str, value: &str) -> Document {
    Document {
        blocks: vec![Block::Keyword {
            name: name.into(),
            value: value.into(),
        }],
    }
}

fn list(list_type: ListType, items: &[&str]) -> Document {
    Document {
        blocks: vec![Block::List {
            list_type,
            items: items
                .iter()
                .map(|text| ListItem {
                    content: vec![Block::Paragraph {
                        inlines: vec![Inline::Plain((*text).into())],
                    }],
                    checkbox: tftio_org::ast::Checkbox::NoCheckbox,
                })
                .collect(),
        }],
    }
}

fn cell(text: &str) -> TableCell {
    TableCell {
        inlines: vec![Inline::Plain(text.into())],
    }
}

fn project(input: &str) -> Option<String> {
    parse_document(input).ok().map(|doc| write_document(&doc))
}

#[test]
fn conforming_cases_match_org_semantics() {
    for case in cases() {
        let Expectation::Conforms(want) = &case.expectation else {
            continue;
        };
        let got = parse_document(case.input).ok();
        assert_eq!(got, Some(want.clone()), "origin: {}", case.origin);
    }
}

#[test]
fn conforming_cases_project_idempotently() {
    for case in cases() {
        let Expectation::Conforms(_) = &case.expectation else {
            continue;
        };
        let Some(first) = project(case.input) else {
            continue;
        };
        let second = project(&first);
        assert_eq!(
            second.as_deref(),
            Some(first.as_str()),
            "origin: {}",
            case.origin
        );
    }
}

#[test]
fn divergent_cases_pin_current_behavior() {
    for case in cases() {
        let Expectation::Diverges { id, doc } = &case.expectation else {
            continue;
        };
        let got = parse_document(case.input).ok();
        assert_eq!(
            got,
            Some(doc.clone()),
            "divergence {id} (origin: {}) changed; flip the corpus entry and its ledger row together",
            case.origin
        );
    }
}

#[test]
fn divergence_ledger_ids_match_corpus_ids() {
    let ledger = include_str!("conformance-DIVERGENCES.md");
    let ledger_ids: BTreeSet<&str> = ledger
        .lines()
        .filter_map(|line| line.split('|').nth(1))
        .map(str::trim)
        .filter(|cell| cell.starts_with("D-"))
        .collect();
    let corpus_ids: BTreeSet<&str> = cases()
        .iter()
        .filter_map(|case| match &case.expectation {
            Expectation::Diverges { id, .. } => Some(*id),
            Expectation::Conforms(_) => None,
        })
        .collect();
    assert_eq!(
        corpus_ids, ledger_ids,
        "corpus and ledger divergence ids must match as sets"
    );
}
