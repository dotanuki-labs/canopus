// Copyright 2025 Dotanuki Labs
// SPDX-License-Identifier: MIT

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_codeowners() -> *const ();
}

pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_codeowners) };

pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

pub fn create_parser() -> tree_sitter::Parser {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&LANGUAGE.into()).expect("failed to load parser");
    parser
}

#[cfg(test)]
mod tests {
    use crate::create_parser;
    use assertor::{EqualityAssertion, assert_that};
    use indoc::indoc;

    #[test]
    fn should_parse_codeowners_file() {
        let mut parser = create_parser();

        let codeowners = indoc! {"
            # A simple codeowners file
            *.rs    @org/crabbers
            /docs   ufs@dotanuki.dev
        "};

        let tree = parser.parse(codeowners, None).unwrap();
        assert_that!(tree.root_node().child_count()).is_equal_to(3);
    }
}
