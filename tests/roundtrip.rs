//! Round-trip contracts between the parser and the writer.
//!
//! Two contracts run over one explicit input table:
//!
//! - **Byte-exact re-projection**: for inputs already in canonical form,
//!   `write_document(parse(x)) == x`.
//! - **Idempotence**: for all inputs, including non-canonical ones,
//!   `write_document(parse(write_document(parse(x)))) == write_document(parse(x))`.
//!
//! Table entries marked `None` for the expected projection deliberately
//! lose content to parse residue (unsupported syntax); only idempotence
//! applies to them. A serde JSON round-trip runs over the same table.

use tftio_org::ast::Document;
use tftio_org::parser::parse_document;
use tftio_org::writer::write_document;

/// `(input, expected first projection)` pairs. `Some` entries assert the
/// writer's first projection byte-for-byte; `None` entries are inputs whose
/// parse residue drops content, so only idempotence applies.
const CASES: &[(&str, Option<&str>)] = &[
    // Canonical inputs: every Block and Inline variant, byte-exact.
    ("* Hello\n", Some("* Hello\n")),
    ("** Task :rust:kb:\n", Some("** Task :rust:kb:\n")),
    (
        "* Parent\nbody\n** Child\n",
        Some("* Parent\nbody\n** Child\n"),
    ),
    ("* One\n\nbody\n\n* Two\n", Some("* One\n\nbody\n\n* Two\n")),
    ("\n\n* Only\n\n\n", Some("\n\n* Only\n\n\n")),
    ("some text\n", Some("some text\n")),
    ("first\nsecond\n", Some("first\nsecond\n")),
    (
        "a *b* c /d/ e +f+ g =h= i ~j~ k [[l]] [[m][n]]\n",
        Some("a *b* c /d/ e +f+ g =h= i ~j~ k [[l]] [[m][n]]\n"),
    ),
    ("*/nested/*\n", Some("*/nested/*\n")),
    ("*Bad\n", Some("*Bad\n")),
    ("=code\n~verb\n", Some("=code\n~verb\n")),
    ("[[target]x\n", Some("[[target]x\n")),
    (
        "#+begin_src rust\nfn main() {}\n#+end_src\n",
        Some("#+begin_src rust\nfn main() {}\n#+end_src\n"),
    ),
    (
        "#+begin_src\nplain\n#+end_src\n",
        Some("#+begin_src\nplain\n#+end_src\n"),
    ),
    (
        "#+begin_example\n$ ls\n#+end_example\n",
        Some("#+begin_example\n$ ls\n#+end_example\n"),
    ),
    (
        "#+begin_quote\nquoted\n#+end_quote\n",
        Some("#+begin_quote\nquoted\n#+end_quote\n"),
    ),
    ("- one\n- two\n", Some("- one\n- two\n")),
    ("- [ ] todo\n- [X] done\n", Some("- [ ] todo\n- [X] done\n")),
    ("- item\n  continuation\n", Some("- item\n  continuation\n")),
    ("1. one\n2. two\n", Some("1. one\n2. two\n")),
    ("2. two\n3. three\n", Some("2. two\n3. three\n")),
    ("| a | b |\n| c | d |\n", Some("| a | b |\n| c | d |\n")),
    ("| a |  | c |\n", Some("| a |  | c |\n")),
    (
        ":PROPERTIES:\n:ID: abc-123\n:END:\n",
        Some(":PROPERTIES:\n:ID: abc-123\n:END:\n"),
    ),
    (
        ":LOGBOOK:\n- <2026-04-30 Thu> noted\n:END:\n",
        Some(":LOGBOOK:\n- <2026-04-30 Thu> noted\n:END:\n"),
    ),
    (
        "SCHEDULED: <2026-05-01> DEADLINE: [2026-05-15]\n",
        Some("SCHEDULED: <2026-05-01> DEADLINE: [2026-05-15]\n"),
    ),
    (
        "CLOSED: [2026-04-30 Thu 10:00]\n",
        Some("CLOSED: [2026-04-30 Thu 10:00]\n"),
    ),
    ("# remark\n", Some("# remark\n")),
    ("#+title: Example\n", Some("#+title: Example\n")),
    ("\n\n-----\n", Some("\n\n-----\n")),
    ("", Some("")),
    // Non-canonical inputs: each exercises a documented normalization rule.
    ("  | a | b |\n", Some("| a | b |\n")),
    ("* Headline     :tag:\n", Some("* Headline :tag:\n")),
    ("para", Some("para\n")),
    ("1. one\n3. three\n", Some("1. one\n2. three\n")),
    (
        "SCHEDULED:  <2026-05-01>   DEADLINE: <b>\n",
        Some("SCHEDULED: <2026-05-01> DEADLINE: <b>\n"),
    ),
    ("#+TITLE: Example\n", Some("#+title: Example\n")),
    (
        "#+BEGIN_SRC rust\nx\n#+END_SRC\n",
        Some("#+begin_src rust\nx\n#+end_src\n"),
    ),
    (
        "#+begin_src rust\nfn x() {}",
        Some("#+begin_src rust\nfn x() {}\n#+end_src\n"),
    ),
    // Residue-losing inputs: only idempotence applies.
    ("#+BEGIN_VERSE\nline one\n#+END_VERSE\n", None),
    ("SCHEDULED: later\n", None),
];

fn project(input: &str) -> Option<String> {
    parse_document(input).ok().map(|doc| write_document(&doc))
}

#[test]
fn canonical_inputs_reproject_byte_exact() {
    for (input, expected) in CASES {
        let Some(expected) = expected else { continue };
        let projected = project(input);
        assert_eq!(projected.as_deref(), Some(*expected), "case: {input:?}");
    }
}

#[test]
fn all_inputs_project_idempotently() {
    let mut parsed = 0;
    for (input, _) in CASES {
        let Some(first) = project(input) else {
            continue;
        };
        parsed += 1;
        let second = project(&first);
        assert_eq!(second.as_deref(), Some(first.as_str()), "case: {input:?}");
    }
    assert_eq!(parsed, CASES.len(), "every case must parse");
}

#[test]
fn documents_survive_serde_json_roundtrip() -> Result<(), serde_json::Error> {
    for (input, _) in CASES {
        let Some(doc) = parse_document(input).ok() else {
            continue;
        };
        let json = serde_json::to_string(&doc)?;
        let back: Document = serde_json::from_str(&json)?;
        assert_eq!(doc, back, "case: {input:?}");
    }
    Ok(())
}
