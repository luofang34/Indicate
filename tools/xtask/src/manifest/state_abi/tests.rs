//! The highest-module rule, exercised against the shapes an `abi.rs`
//! can take. The guard exists to catch a module added past this
//! generator, so the case that matters is a source declaring more than
//! one version.

#![allow(clippy::expect_used, clippy::panic)]

use super::newest_module;

#[test]
fn reads_the_only_declared_module() {
    assert_eq!(newest_module("pub mod v7;\n"), Some(7));
}

#[test]
fn takes_the_highest_of_several() {
    let source = "//! doc\npub mod v7;\npub mod v8;\npub mod v5;\n";
    assert_eq!(newest_module(source), Some(8));
}

#[test]
fn compares_numerically_not_lexically() {
    assert_eq!(newest_module("pub mod v9;\npub mod v10;\n"), Some(10));
}

#[test]
fn ignores_unversioned_modules() {
    let source = "pub mod version;\nmod vault;\npub mod v7;\n";
    assert_eq!(newest_module(source), Some(7));
}

#[test]
fn a_source_declaring_none_answers_none() {
    assert_eq!(
        newest_module("//! no modules here\npub const X: u8 = 1;\n"),
        None
    );
}

#[test]
fn the_shipped_crate_declares_the_module_this_generator_reads() {
    let root = crate::workspace::repo_root();
    let abi = super::read(&root).expect("the shipped abi.rs declares the compiled module");
    assert_eq!(abi.module, format!("v{}", super::COMPILED_MODULE));
    assert_eq!(abi.version, indicate_instrument_state::abi::v8::VERSION);
}

/// A declaration is not hidden by anything following the semicolon. A
/// scan that missed one would certify an ABI the tree stopped shipping,
/// which is the failure this guard exists to prevent.
#[test]
fn a_trailing_comment_does_not_hide_a_newer_module() {
    for line in [
        "pub mod v8;",
        "pub mod v8; // staged",
        "pub mod v8;// staged",
        "    pub mod v8;\t// indented and tabbed",
    ] {
        let source = std::format!("pub mod v7;\n{line}\n");
        assert_eq!(
            super::newest_module(&source),
            Some(8),
            "{line:?} declares v8"
        );
    }
}

/// Only a versioned module counts: a neighbouring declaration must not
/// be read as one.
#[test]
fn only_versioned_modules_are_counted() {
    for line in ["pub mod validate;", "pub mod v;", "pub mod v8x;"] {
        assert_eq!(
            super::newest_module(line),
            None,
            "{line:?} is not a version"
        );
    }
}
