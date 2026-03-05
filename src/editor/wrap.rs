use ropey::{Rope, RopeSlice};
use unicode_width::UnicodeWidthChar;

use crate::buffer;

/// A single screen line segment produced by wrapping a document line.
#[derive(Debug, Clone)]
pub struct WrapSegment {
    pub doc_row: usize,
    /// 0 = first segment of the line
    pub segment_index: usize,
    /// Start character index in the document line (inclusive)
    pub char_start: usize,
    /// End character index in the document line (exclusive)
    pub char_end: usize,
}

/// Display width of a single character (tab counts as 1).
fn char_width(ch: char) -> u16 {
    if ch == '\t' {
        1
    } else {
        UnicodeWidthChar::width(ch).unwrap_or(1) as u16
    }
}

/// How many screen lines a document line occupies when wrapped.
/// Empty lines and lines fitting within text_width occupy 1 screen line.
pub fn wrap_count(line: RopeSlice, text_width: u16) -> usize {
    if text_width == 0 {
        return 1;
    }
    let line_len = buffer::line_display_len(line);
    if line_len == 0 {
        return 1;
    }
    let mut segments = 1usize;
    let mut col: u16 = 0;
    for i in 0..line_len {
        let ch = line.char(i);
        let w = char_width(ch);
        // CJK character that doesn't fit on this segment → start new segment
        if w > 1 && col + w > text_width && col > 0 {
            segments += 1;
            col = w;
            continue;
        }
        if col + w > text_width {
            segments += 1;
            col = w;
        } else {
            col += w;
        }
    }
    segments
}

/// Convert a character index within a line to (segment_index, display_column_within_segment).
pub fn char_to_wrap_pos(line: RopeSlice, char_idx: usize, text_width: u16) -> (usize, u16) {
    if text_width == 0 {
        return (0, 0);
    }
    let line_len = buffer::line_display_len(line);
    let target = char_idx.min(line_len);
    let mut segment = 0usize;
    let mut col: u16 = 0;
    for i in 0..target {
        let ch = line.char(i);
        let w = char_width(ch);
        if w > 1 && col + w > text_width && col > 0 {
            segment += 1;
            col = w;
            continue;
        }
        if col + w > text_width {
            segment += 1;
            col = w;
        } else {
            col += w;
        }
    }
    (segment, col)
}

/// Convert (segment_index, target_display_column) back to a character index.
/// Used for vertical cursor movement within wrapped lines.
pub fn wrap_pos_to_char(line: RopeSlice, segment: usize, target_col: u16, text_width: u16) -> usize {
    if text_width == 0 {
        return 0;
    }
    let line_len = buffer::line_display_len(line);
    if line_len == 0 {
        return 0;
    }
    let mut cur_segment = 0usize;
    let mut col: u16 = 0;
    let mut seg_start_char = 0usize;

    for i in 0..line_len {
        if cur_segment == segment {
            // We're on the target segment
            let ch = line.char(i);
            let w = char_width(ch);
            // Check if this character would push us to the next segment
            let would_wrap = if w > 1 && col + w > text_width && col > 0 {
                true
            } else {
                col + w > text_width
            };
            if would_wrap {
                // We've run out of space on this segment without reaching target_col.
                // Return the last char in this segment.
                return i.saturating_sub(1).max(seg_start_char);
            }
            if col >= target_col {
                return i;
            }
            col += w;
        } else {
            let ch = line.char(i);
            let w = char_width(ch);
            if w > 1 && col + w > text_width && col > 0 {
                cur_segment += 1;
                col = w;
                if cur_segment == segment {
                    seg_start_char = i;
                    col = w;
                    if target_col == 0 {
                        return i;
                    }
                }
                continue;
            }
            if col + w > text_width {
                cur_segment += 1;
                col = w;
                if cur_segment == segment {
                    seg_start_char = i;
                    col = w;
                    if target_col == 0 {
                        return i;
                    }
                }
            } else {
                col += w;
            }
        }
    }
    // If segment is beyond available segments, return end of line
    if cur_segment < segment {
        return line_len.saturating_sub(1);
    }
    // Past end of segment — clamp to last char
    line_len.saturating_sub(1)
}

/// Build a screen map of WrapSegments starting from (start_doc_row, start_wrap_segment)
/// for up to screen_height screen lines.
pub fn build_screen_map(
    rope: &Rope,
    start_doc_row: usize,
    start_wrap_segment: usize,
    text_width: u16,
    screen_height: u16,
) -> Vec<WrapSegment> {
    let mut result = Vec::with_capacity(screen_height as usize);
    let line_count = rope.len_lines();
    let mut doc_row = start_doc_row;

    if doc_row >= line_count {
        return result;
    }

    // For the first line, we may skip some segments
    let first_line = rope.line(doc_row);
    let segments = build_line_segments(first_line, doc_row, text_width);
    for seg in segments.into_iter().skip(start_wrap_segment) {
        result.push(seg);
        if result.len() >= screen_height as usize {
            return result;
        }
    }
    doc_row += 1;

    while doc_row < line_count && result.len() < screen_height as usize {
        let line = rope.line(doc_row);
        let segments = build_line_segments(line, doc_row, text_width);
        for seg in segments {
            result.push(seg);
            if result.len() >= screen_height as usize {
                return result;
            }
        }
        doc_row += 1;
    }

    result
}

/// Build all WrapSegments for a single document line.
fn build_line_segments(line: RopeSlice, doc_row: usize, text_width: u16) -> Vec<WrapSegment> {
    let line_len = buffer::line_display_len(line);
    if line_len == 0 || text_width == 0 {
        return vec![WrapSegment {
            doc_row,
            segment_index: 0,
            char_start: 0,
            char_end: 0,
        }];
    }

    let mut segments = Vec::new();
    let mut seg_start = 0usize;
    let mut seg_idx = 0usize;
    let mut col: u16 = 0;

    for i in 0..line_len {
        let ch = line.char(i);
        let w = char_width(ch);

        let need_wrap = if w > 1 && col + w > text_width && col > 0 {
            true
        } else {
            col + w > text_width
        };

        if need_wrap {
            segments.push(WrapSegment {
                doc_row,
                segment_index: seg_idx,
                char_start: seg_start,
                char_end: i,
            });
            seg_idx += 1;
            seg_start = i;
            col = w;
        } else {
            col += w;
        }
    }

    // Final segment
    segments.push(WrapSegment {
        doc_row,
        segment_index: seg_idx,
        char_start: seg_start,
        char_end: line_len,
    });

    segments
}
