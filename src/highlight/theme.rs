use ratatui::style::{Color, Modifier, Style};

/// One Dark inspired color scheme.
/// Maps tree-sitter capture names to ratatui styles.
pub fn style_for_capture(name: &str) -> Style {
    match name {
        "keyword" | "keyword.control" | "keyword.control.rust" | "keyword.modifier"
        | "keyword.type" | "keyword.function" | "keyword.operator" | "keyword.import"
        | "keyword.repeat" | "keyword.return" | "keyword.conditional" | "keyword.exception"
        | "keyword.storage" | "keyword.coroutine" | "keyword.directive" => {
            Style::default().fg(Color::Rgb(198, 120, 221)) // purple
        }
        "type" | "type.builtin" | "type.qualifier" => {
            Style::default().fg(Color::Rgb(229, 192, 123)) // yellow
        }
        "constructor" => Style::default().fg(Color::Rgb(229, 192, 123)), // yellow
        "function" | "function.call" | "function.method" | "function.method.call"
        | "function.macro" | "function.builtin" => {
            Style::default().fg(Color::Rgb(97, 175, 239)) // blue
        }
        "string" | "string.special" | "string.escape" | "string.regexp" => {
            Style::default().fg(Color::Rgb(152, 195, 121)) // green
        }
        "character" | "character.special" => {
            Style::default().fg(Color::Rgb(152, 195, 121)) // green
        }
        "number" | "number.float" | "float" | "boolean" | "constant.builtin" => {
            Style::default().fg(Color::Rgb(209, 154, 102)) // orange
        }
        "comment" | "comment.line" | "comment.block" | "comment.documentation" => {
            Style::default()
                .fg(Color::Rgb(92, 99, 112))
                .add_modifier(Modifier::ITALIC) // gray italic
        }
        "variable.builtin" | "variable.parameter" => {
            Style::default().fg(Color::Rgb(224, 108, 117)) // red
        }
        "constant" => Style::default().fg(Color::Rgb(209, 154, 102)), // orange
        "attribute" | "attribute.builtin" => {
            Style::default().fg(Color::Rgb(229, 192, 123)) // yellow
        }
        "label" => {
            Style::default().fg(Color::Rgb(209, 154, 102)) // orange
        }
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter"
        | "punctuation.special" => {
            Style::default().fg(Color::Rgb(171, 178, 191)) // light gray
        }
        "operator" => Style::default().fg(Color::Rgb(171, 178, 191)), // light gray
        "property" | "variable.member" => {
            Style::default().fg(Color::Rgb(224, 108, 117)) // red
        }
        "escape" | "string.special.symbol" => {
            Style::default().fg(Color::Rgb(86, 182, 194)) // cyan
        }
        "module" | "namespace" => {
            Style::default().fg(Color::Rgb(229, 192, 123)) // yellow
        }
        _ => Style::default().fg(Color::Rgb(171, 178, 191)), // default light gray
    }
}

/// Default text color (used when no capture matches).
pub fn default_style() -> Style {
    Style::default().fg(Color::Rgb(171, 178, 191))
}
