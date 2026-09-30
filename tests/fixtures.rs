//! Whole-file fixture corpus.
//!
//! Each fixture in `tests/fixtures/` is a realistic org file authored for
//! this repository. The manifest below declares, per fixture, whether it is
//! canonical (byte-exact under parse-then-project) and the exact parse
//! residue expected — declared here, never inferred from parser behavior.

use tftio_org::ast::Document;
use tftio_org::parser::parse_document_with_residue;
use tftio_org::writer::write_document;

struct Fixture {
    name: &'static str,
    /// Whether `write_document(parse(content)) == content` must hold.
    canonical: bool,
    /// Lines no block is expected to claim, in order.
    expected_residue: &'static [&'static str],
    content: &'static str,
}

const MANIFEST: &[Fixture] = &[
    Fixture {
        name: "kb-note.org",
        canonical: true,
        expected_residue: &[],
        content: include_str!("fixtures/kb-note.org"),
    },
    Fixture {
        name: "article.org",
        canonical: false,
        expected_residue: &["#+BEGIN_VERSE"],
        content: include_str!("fixtures/article.org"),
    },
    Fixture {
        name: "planning-logbook.org",
        canonical: true,
        expected_residue: &[],
        content: include_str!("fixtures/planning-logbook.org"),
    },
    Fixture {
        name: "syntax-torture.org",
        canonical: false,
        expected_residue: &["#+BEGIN_CENTER"],
        content: include_str!("fixtures/syntax-torture.org"),
    },
    Fixture {
        name: "minimal.org",
        canonical: true,
        expected_residue: &[],
        content: include_str!("fixtures/minimal.org"),
    },
];

fn parse(input: &str) -> Option<(Document, Vec<String>)> {
    parse_document_with_residue(input).ok()
}

fn project(input: &str) -> Option<String> {
    parse(input).map(|(doc, _)| write_document(&doc))
}

#[test]
fn fixture_residue_matches_the_manifest() {
    let mut parsed = 0;
    for fixture in MANIFEST {
        let Some((_, residue)) = parse(fixture.content) else {
            continue;
        };
        parsed += 1;
        assert_eq!(
            residue, fixture.expected_residue,
            "fixture {}",
            fixture.name
        );
        assert!(
            !fixture.content.contains('\r'),
            "fixture {} must be LF-only",
            fixture.name
        );
    }
    assert_eq!(parsed, MANIFEST.len(), "every fixture must parse");
}

#[test]
fn canonical_fixtures_reproject_byte_exact() {
    for fixture in MANIFEST.iter().filter(|fixture| fixture.canonical) {
        let projected = project(fixture.content);
        assert_eq!(
            projected.as_deref(),
            Some(fixture.content),
            "fixture {}",
            fixture.name
        );
    }
}

#[test]
fn all_fixtures_project_idempotently() {
    for fixture in MANIFEST {
        let Some(first) = project(fixture.content) else {
            continue;
        };
        let second = project(&first);
        assert_eq!(
            second.as_deref(),
            Some(first.as_str()),
            "fixture {}",
            fixture.name
        );
    }
}
