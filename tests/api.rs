//! Public API integration tests for `tftio-org`.

use tftio_org::{
    ast::{Block, Document, Inline, Title},
    parser::parse_document,
};

#[test]
fn parser_and_ast_are_exposed_at_the_expected_paths() {
    assert_eq!(
        parse_document("* Heading\nbody\n").ok(),
        Some(Document {
            blocks: vec![Block::Heading {
                level: 1,
                title: Title("Heading".into()),
                tags: vec![],
                children: vec![Block::Paragraph {
                    inlines: vec![Inline::Plain("body".into())],
                }],
            }],
        })
    );
}
