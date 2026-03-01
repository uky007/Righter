use super::style::{RgbColor, SyntaxStyle};

/// One Dark inspired color scheme.
/// Maps tree-sitter capture names to SyntaxStyle.
pub fn style_for_capture(name: &str) -> SyntaxStyle {
    match name {
        "keyword" | "keyword.control" | "keyword.control.rust" | "keyword.modifier"
        | "keyword.type" | "keyword.function" | "keyword.operator" | "keyword.import"
        | "keyword.repeat" | "keyword.return" | "keyword.conditional" | "keyword.exception"
        | "keyword.storage" | "keyword.coroutine" | "keyword.directive" => {
            SyntaxStyle::default().fg(RgbColor(198, 120, 221)) // purple
        }
        "type" | "type.builtin" | "type.qualifier" => {
            SyntaxStyle::default().fg(RgbColor(229, 192, 123)) // yellow
        }
        "constructor" => SyntaxStyle::default().fg(RgbColor(229, 192, 123)), // yellow
        "function" | "function.call" | "function.method" | "function.method.call"
        | "function.macro" | "function.builtin" => {
            SyntaxStyle::default().fg(RgbColor(97, 175, 239)) // blue
        }
        "string" | "string.special" | "string.escape" | "string.regexp" => {
            SyntaxStyle::default().fg(RgbColor(152, 195, 121)) // green
        }
        "character" | "character.special" => {
            SyntaxStyle::default().fg(RgbColor(152, 195, 121)) // green
        }
        "number" | "number.float" | "float" | "boolean" | "constant.builtin" => {
            SyntaxStyle::default().fg(RgbColor(209, 154, 102)) // orange
        }
        "comment" | "comment.line" | "comment.block" | "comment.documentation" => {
            SyntaxStyle::default()
                .fg(RgbColor(92, 99, 112))
                .italic() // gray italic
        }
        "variable.builtin" | "variable.parameter" => {
            SyntaxStyle::default().fg(RgbColor(224, 108, 117)) // red
        }
        "constant" => SyntaxStyle::default().fg(RgbColor(209, 154, 102)), // orange
        "attribute" | "attribute.builtin" => {
            SyntaxStyle::default().fg(RgbColor(229, 192, 123)) // yellow
        }
        "label" => {
            SyntaxStyle::default().fg(RgbColor(209, 154, 102)) // orange
        }
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter"
        | "punctuation.special" => {
            SyntaxStyle::default().fg(RgbColor(171, 178, 191)) // light gray
        }
        "operator" => SyntaxStyle::default().fg(RgbColor(171, 178, 191)), // light gray
        "property" | "variable.member" => {
            SyntaxStyle::default().fg(RgbColor(224, 108, 117)) // red
        }
        "escape" | "string.special.symbol" => {
            SyntaxStyle::default().fg(RgbColor(86, 182, 194)) // cyan
        }
        "module" | "namespace" => {
            SyntaxStyle::default().fg(RgbColor(229, 192, 123)) // yellow
        }
        _ => SyntaxStyle::default().fg(RgbColor(171, 178, 191)), // default light gray
    }
}

/// Default text color (used when no capture matches).
pub fn default_style() -> SyntaxStyle {
    SyntaxStyle::default().fg(RgbColor(171, 178, 191))
}
