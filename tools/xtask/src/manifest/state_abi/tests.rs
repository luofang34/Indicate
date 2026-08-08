//! The highest-module rule, exercised against the shapes an `abi.rs`
//! can take. The guard exists to catch a module added past this
//! generator, so the case that matters is a source declaring more than
//! one version.

#![allow(clippy::expect_used, clippy::panic)]

use super::newest_module;

#[test]
fn reads_the_only_declared_module() {
    assert_eq!(newest_module("pub mod v6;\n"), Some(6));
}

#[test]
fn takes_the_highest_of_several() {
    let source = "//! doc\npub mod v6;\npub mod v7;\npub mod v5;\n";
    assert_eq!(newest_module(source), Some(7));
}

#[test]
fn compares_numerically_not_lexically() {
    assert_eq!(newest_module("pub mod v9;\npub mod v10;\n"), Some(10));
}

#[test]
fn ignores_unversioned_modules() {
    let source = "pub mod version;\nmod vault;\npub mod v6;\n";
    assert_eq!(newest_module(source), Some(6));
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
    assert_eq!(abi.version, indicate_instrument_state::abi::v6::VERSION);
}
