use frac_lang::ast::{File, ParseError};
use frac_lang::normalizer::{normalize, NormalizedFile, NormalizeError};
use frac_lang::parser::parse;

use crate::line_index::LineIndex;

pub struct Document {
    pub line_index: LineIndex,
    pub file: File,
    pub parse_errors: Vec<ParseError>,
    pub normalized: Option<NormalizedFile>,
    pub normalize_errors: Vec<NormalizeError>,
}

impl Document {
    pub fn build(source: String) -> Self {
        let line_index = LineIndex::new(&source);
        let (file, parse_errors) = parse(&source);
        let (normalized, normalize_errors) = match normalize(file.clone()) {
            Ok(nf) => (Some(nf), Vec::new()),
            Err(errs) => (None, errs),
        };
        Self { line_index, file, parse_errors, normalized, normalize_errors }
    }
}
