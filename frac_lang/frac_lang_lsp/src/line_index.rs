use frac_lang::ast::Span;
use tower_lsp::lsp_types::{Position, Range};

pub struct LineIndex {
    line_starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts, len: text.len() }
    }

    pub fn position_of_offset(&self, offset: usize) -> Position {
        let offset = offset.min(self.len);
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line];
        Position {
            line: line as u32,
            character: (offset - line_start) as u32,
        }
    }

    pub fn offset_of_position(&self, pos: Position) -> usize {
        let line = (pos.line as usize).min(self.line_starts.len().saturating_sub(1));
        let line_start = self.line_starts[line];
        let line_end = self.line_starts.get(line + 1).copied().unwrap_or(self.len);
        (line_start + pos.character as usize).min(line_end)
    }

    pub fn range_of_span(&self, span: Span) -> Range {
        Range {
            start: self.position_of_offset(span.start),
            end: self.position_of_offset(span.end),
        }
    }
}
