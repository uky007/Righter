pub mod style;
pub mod theme;

use ropey::Rope;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor, Tree};

use self::style::SyntaxStyle;
use self::theme::{default_style, style_for_capture};

/// Per-line highlight spans: Vec of (start_col, end_col, SyntaxStyle) per visible line.
pub type LineStyles = Vec<Vec<(usize, usize, SyntaxStyle)>>;

pub struct Highlighter {
    parser: Parser,
    query: Query,
}

impl Highlighter {
    pub fn new() -> Option<Self> {
        let language = tree_sitter_rust::LANGUAGE;
        let mut parser = Parser::new();
        parser.set_language(&language.into()).ok()?;

        // Use the bundled highlights query from tree-sitter-rust.
        let query_source = tree_sitter_rust::HIGHLIGHTS_QUERY;
        let query = Query::new(&language.into(), query_source).ok()?;

        Some(Self { parser, query })
    }

    /// Parse (or reparse) the document. Returns a new syntax tree.
    pub fn parse(&mut self, rope: &Rope, old_tree: Option<&Tree>) -> Option<Tree> {
        self.parser.parse_with(
            &mut |byte_offset: usize, _position| -> &[u8] {
                if byte_offset >= rope.len_bytes() {
                    return &[];
                }
                let (chunk, chunk_byte_start, _, _) = rope.chunk_at_byte(byte_offset);
                &chunk.as_bytes()[byte_offset - chunk_byte_start..]
            },
            old_tree,
        )
    }

    /// Compute highlight spans for the given line range [start_line, end_line).
    pub fn highlight_lines(
        &self,
        tree: &Tree,
        rope: &Rope,
        start_line: usize,
        end_line: usize,
    ) -> LineStyles {
        let num_lines = end_line.saturating_sub(start_line);
        let mut result: Vec<Vec<(usize, usize, SyntaxStyle)>> = vec![vec![]; num_lines];

        let source = rope.to_string();
        let source_bytes = source.as_bytes();

        let start_byte = rope.line_to_byte(start_line);
        let end_byte = if end_line < rope.len_lines() {
            rope.line_to_byte(end_line)
        } else {
            rope.len_bytes()
        };

        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(start_byte..end_byte);

        let capture_names = self.query.capture_names();
        let mut captures = cursor.captures(&self.query, tree.root_node(), source_bytes);

        while let Some(&(ref match_, capture_idx)) = captures.next() {
            let capture = &match_.captures[capture_idx];
            let name = capture_names[capture.index as usize];
            let style = style_for_capture(name);

            let node = capture.node;
            let start_pos = node.start_position();
            let end_pos = node.end_position();

            for line in start_pos.row..=end_pos.row {
                if line < start_line || line >= end_line {
                    continue;
                }
                let rel_line = line - start_line;

                let col_start = if line == start_pos.row {
                    byte_col_to_char_col(rope, line, start_pos.column)
                } else {
                    0
                };

                let col_end = if line == end_pos.row {
                    byte_col_to_char_col(rope, line, end_pos.column)
                } else {
                    rope.line(line).len_chars()
                };

                if col_start < col_end {
                    result[rel_line].push((col_start, col_end, style));
                }
            }
        }

        result
    }
}

/// Convert a byte column offset within a line to a char column offset.
fn byte_col_to_char_col(rope: &Rope, line: usize, byte_col: usize) -> usize {
    let line_byte_start = rope.line_to_byte(line);
    let abs_byte = line_byte_start + byte_col;
    let abs_byte = abs_byte.min(rope.len_bytes());
    let abs_char = rope.byte_to_char(abs_byte);
    let line_char_start = rope.line_to_char(line);
    abs_char.saturating_sub(line_char_start)
}

/// Look up the highlight style for a specific position.
pub fn style_at(line_styles: &[Vec<(usize, usize, SyntaxStyle)>], rel_line: usize, col: usize) -> SyntaxStyle {
    if rel_line < line_styles.len() {
        let mut result = default_style();
        for &(start, end, style) in &line_styles[rel_line] {
            if col >= start && col < end {
                result = style;
            }
        }
        result
    } else {
        default_style()
    }
}
