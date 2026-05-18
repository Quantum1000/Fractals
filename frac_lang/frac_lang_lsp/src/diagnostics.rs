use frac_lang::ast::Span;
use frac_lang::normalizer::NormalizeError;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

use crate::document::Document;

pub fn collect(doc: &Document) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for e in &doc.parse_errors {
        diags.push(Diagnostic {
            range: doc.line_index.range_of_span(e.span),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("frac-parse".into()),
            message: e.message.clone(),
            ..Default::default()
        });
    }

    for e in &doc.normalize_errors {
        for (span, message) in split_normalize_error(e) {
            diags.push(Diagnostic {
                range: doc.line_index.range_of_span(span),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("frac-normalize".into()),
                message,
                ..Default::default()
            });
        }
    }

    diags
}

fn split_normalize_error(e: &NormalizeError) -> Vec<(Span, String)> {
    use NormalizeError::*;
    let msg = format!("{e}");
    let span = match e {
        ParseErrors(errs) => return errs.iter()
            .map(|p| (p.span, format!("parse error: {}", p.message)))
            .collect(),
        DuplicateTile { span, .. } => *span,
        DuplicatePartition { span, .. } => *span,
        UnknownTile { span, .. } => *span,
        UnknownPartition { span, .. } => *span,
        InconsistentSymmetry { span, .. } => *span,
        NoTileMatch { span, .. } => *span,
        PartitionIncomplete { span, .. } => *span,
        ChildNameOutOfRange { span, .. } => *span,
        TypeMismatch { span, .. } => *span,
        NullableColor { span, .. } => *span,
        RecursiveFunction { .. } => Span { start: 0, end: 0 },
        UnknownFunction { span, .. } => *span,
        UnknownVariable { span, .. } => *span,
        InvalidSlotOrder { span, .. } => *span,
        WrongArgCount { span, .. } => *span,
        InvalidPartitionVertex { span, .. } => *span,
        DuplicateFunction { span, .. } => *span,
        DuplicateVertex { span, .. } => *span,
        DuplicateState { span, .. } => *span,
        DuplicateParameter { span, .. } => *span,
        TileWithoutPartition { span, .. } => *span,
    };
    vec![(span, msg)]
}
