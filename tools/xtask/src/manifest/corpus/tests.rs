//! The header reader, against the shapes a corpus JSON can take. A
//! per-entry key of the same name must not answer for the document, and
//! an ambiguous header must not be resolved by position.

#![allow(clippy::expect_used, clippy::panic)]

use super::{header_value, read};

const HEADER: &str = concat!(
    "{\n",
    "  \"schemaVersion\": 2,\n",
    "  \"corpusVersion\": 4,\n",
    "  \"corpusSha256\": \"aa\",\n",
    "  \"entries\": [\n",
    "    {\n",
    "      \"corpusVersion\": 99\n",
    "    }\n",
    "  ]\n",
    "}\n"
);

#[test]
fn reads_a_header_value() {
    let value = header_value(HEADER, "corpusVersion", "test").expect("header states the version");
    assert_eq!(value, "4");
}

#[test]
fn a_nested_key_does_not_answer_for_the_document() {
    let entries_only = "{\n  \"entries\": [\n    { \"corpusVersion\": 99 }\n  ]\n}\n";
    assert!(header_value(entries_only, "corpusVersion", "test").is_err());
}

#[test]
fn a_repeated_header_key_is_refused() {
    let doubled = "{\n  \"corpusVersion\": 4,\n  \"corpusVersion\": 5,\n  \"entries\": []\n}\n";
    assert!(header_value(doubled, "corpusVersion", "test").is_err());
}

#[test]
fn an_absent_key_is_refused() {
    assert!(header_value(HEADER, "corpusFlavour", "test").is_err());
}

#[test]
fn the_shipped_corpus_answers_with_a_version_and_a_hash() {
    let corpus = read(&crate::workspace::repo_root()).expect("the shipped corpus states its pin");
    assert!(corpus.version > 0);
    assert_eq!(corpus.sha256.len(), 64);
}
