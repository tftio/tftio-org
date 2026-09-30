# Conformance divergences

Each id records a place where `tftio-org`'s parse differs from org-mode's
specified semantics, with the upstream citation and the reason it stands.
`tests/conformance.rs` pins each one as a characterization entry; the
accounting test asserts that the corpus ids and the ledger ids match as
sets, so a divergence can appear or disappear only by changing both in the
same commit.

Closing a divergence means flipping its corpus entry to `Conforms` with the
org-specified expectation and deleting its ledger row in the same change.

| Id | Upstream citation | Divergence | Why it stands |
|---|---|---|---|
| D-TODO-KEYWORD | `test-org-element/headline-todo-keyword` | org splits a leading TODO keyword into `:todo-keyword` and strips it from the raw value; the title keeps it verbatim. | TODO-state semantics are a standing non-goal of the crate; the raw title is the model. |
| D-COMMENT-KEYWORD | `test-org-element/headline-comment-keyword` | org recognizes a leading `COMMENT` keyword and sets `:commentedp`; the title keeps it verbatim. | Same as D-TODO-KEYWORD: no commented-heading semantics in scope. |
| D-PRIORITY-COOKIE | `test-org-element/interpret-data` (priority cookies) | org parses `[#A]`-style priority cookies off the title; the title keeps the cookie verbatim. | Priority semantics are out of scope; the raw title is the model. |
| D-EXAMPLE-COMMA-ESCAPE | `test-org-element/example-block-parser` (un-escaping) | org un-escapes leading commas in example bodies (`,*` reads as `*`); the body is kept verbatim with commas. | Comma escaping is an export-editing feature not yet built. |
| D-UNTERMINATED-EXAMPLE | `test-org-element/example-block-parser` (incomplete block) | org does not recognize a `#+BEGIN_EXAMPLE` without its end marker; the parser absorbs the rest of the input as block content. | Deliberate leniency: content is preserved in the AST rather than rejected; already pinned by parser unit tests. |
| D-SRC-SWITCHES | `test-org-element/block-switches` | org parses src/example switches (`-n`, `-r`, `-i`, `-l "fmt"`) and header arguments (`:exports`, `:tangle`) into properties; the whole remainder after the language is kept in the `language` string. | Header-argument model not built; verbatim preservation round-trips. |
| D-GENERAL-DRAWERS | `test-org-element` (drawer parser) | org parses any `:NAME:` drawer; only `:PROPERTIES:` and `:LOGBOOK:` are recognized, and other drawers degrade to paragraph text. | General drawers are not yet built; the degradation is visible rather than silent because the text survives. |
| D-CLOCK-LINE | `test-org-element/interpret-data` (clock) | org parses `CLOCK:` lines with timestamps and durations; they parse as paragraph text. | CLOCK lines and LOGBOOK state entries are not yet built. |
| D-FIXED-WIDTH | `test-org-element/interpret-data` (fixed width) | org parses `: `-prefixed lines as fixed-width elements; they parse as paragraph text. | Fixed-width elements are not yet built. |
| D-RULE-LENGTH | `test-org-element/interpret-data` (horizontal rule) | org accepts five or more hyphens as a horizontal rule; exactly five hyphens are required, and longer runs parse as paragraph text. | The rule grammar has not yet been widened. |
| D-KEYWORD-CASE | `test-org-element/interpret-data` (keywords) | org preserves the written case of a keyword name in `:key`; names are normalized to lowercase. | Intentional normalization so downstream matching never sees `#+TITLE` and `#+title` as different keywords; documented in the writer contract. |
| D-BARE-BULLET | `test-org-element/interpret-data` (lists) | org accepts a bare `-` as an empty list item; a bullet requires `- ` with trailing content, so `-` parses as paragraph text. | The bullet grammar has not yet been widened. |
| D-ORDERED-PAREN | `test-org-element/interpret-data` (lists) | org accepts `1)` as an ordered list marker; only `1.` is recognized, so `1) Item` parses as paragraph text. | Same as D-BARE-BULLET. |
| D-COUNTER-COOKIE | `test-org-element/interpret-data` (lists) | org parses `[@5]` counter cookies and restarts item numbering; the cookie stays in the item text and the list keeps its source ordinal. | Counter cookies are not yet built. |
| D-ITEM-BLOCKS | `test-org-element/interpret-data` (lists) | org allows blocks (tables, paragraphs) as list item content; items hold one paragraph and a following indented block ends the list. | Structural: the flat list model predates block-carrying items. |
| D-DESCRIPTION-LIST | `test-org-element/interpret-data` (lists) | org parses `- term :: definition` as a description item; the `::` stays in the item text. | Description lists are not yet built. |
