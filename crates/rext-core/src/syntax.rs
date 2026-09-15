use std::collections::HashMap;
use std::ops::Range;
use std::sync::Mutex;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Keyword,
    String,
    Comment,
    Number,
    Type,
    Function,
    Variable,
    Operator,
    Property,
    Punctuation,
}

pub const HIGHLIGHT_NAMES: &[&str] = &[
    "keyword",
    "string",
    "comment",
    "number",
    "type",
    "function",
    "variable",
    "operator",
    "property",
    "punctuation",
];

fn scope_for(name: &str) -> Option<Scope> {
    let base = name.split('.').next().unwrap_or(name);
    match base {
        "keyword" => Some(Scope::Keyword),
        "string" => Some(Scope::String),
        "comment" => Some(Scope::Comment),
        "number" => Some(Scope::Number),
        "type" => Some(Scope::Type),
        "function" => Some(Scope::Function),
        "variable" => Some(Scope::Variable),
        "operator" => Some(Scope::Operator),
        "property" => Some(Scope::Property),
        "punctuation" => Some(Scope::Punctuation),
        _ => None,
    }
}

struct Language {
    config: HighlightConfiguration,
}

pub struct SyntaxRegistry {
    languages: HashMap<&'static str, Language>,
    highlighter: Mutex<Highlighter>,
}

impl SyntaxRegistry {
    pub fn new() -> Self {
        let mut languages = HashMap::new();
        languages.insert("rust", Language {
            config: rust_config(),
        });
        languages.insert("json", Language {
            config: json_config(),
        });
        Self {
            languages,
            highlighter: Mutex::new(Highlighter::new()),
        }
    }

    pub fn language_for_path(&self, path: &std::path::Path) -> Option<&'static str> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some("rust"),
            "json" | "jsonc" => Some("json"),
            _ => None,
        }
    }

    pub fn highlight(
        &self,
        lang: &str,
        text: &str,
    ) -> Vec<(Range<usize>, Scope)> {
        let Some(language) = self.languages.get(lang) else {
            return Vec::new();
        };
        let Ok(mut highlighter) = self.highlighter.lock() else {
            return Vec::new();
        };
        let Ok(events) = highlighter.highlight(
            &language.config,
            text.as_bytes(),
            None,
            |_| None,
        ) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut scope_stack: Vec<Scope> = Vec::new();
        for event in events {
            let Ok(event) = event else {
                continue;
            };
            match event {
                HighlightEvent::Source { start, end } => {
                    if let Some(scope) = scope_stack.last() {
                        out.push((start..end, *scope));
                    }
                }
                HighlightEvent::HighlightStart(s) => {
                    let name = HIGHLIGHT_NAMES.get(s.0).copied().unwrap_or("");
                    if let Some(scope) = scope_for(name) {
                        scope_stack.push(scope);
                    }
                }
                HighlightEvent::HighlightEnd => {
                    scope_stack.pop();
                }
            }
        }
        out
    }
}

impl Default for SyntaxRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn rust_config() -> HighlightConfiguration {
    let mut config = HighlightConfiguration::new(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        RUST_HIGHLIGHTS,
        "",
        "",
    )
    .expect("rust highlights query");
    config.configure(HIGHLIGHT_NAMES);
    config
}

fn json_config() -> HighlightConfiguration {
    let mut config = HighlightConfiguration::new(
        tree_sitter_json::LANGUAGE.into(),
        "json",
        JSON_HIGHLIGHTS,
        "",
        "",
    )
    .expect("json highlights query");
    config.configure(HIGHLIGHT_NAMES);
    config
}

const RUST_HIGHLIGHTS: &str = r#"
(line_comment) @comment
(block_comment) @comment
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(boolean_literal) @number
(integer_literal) @number
(float_literal) @number
[
  "fn" "let" "const" "static" "struct" "enum" "impl" "trait"
  "pub" "use" "mod"
  "if" "else" "match" "for" "while" "loop" "in" "return"
  "break" "continue" "where" "async" "await" "move" "ref"
  "type" "union" "unsafe" "extern" "dyn" "as" "try" "default" "gen" "yield"
] @keyword
(type_identifier) @type
(primitive_type) @type.builtin
(self) @variable.special
(field_identifier) @property
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))
(function_item name: (identifier) @function)
(macro_invocation macro: (identifier) @function)
(lifetime) @type
[
  "=" "+" "-" "*" "/" "%" "!" "==" "!=" "<" ">" "<=" ">="
  "&&" "||" "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%="
  "->" "=>" "::" ":" ";" "," "." ".." "..=" "@" "_" "?"
] @operator
[
  "(" ")" "[" "]" "{" "}"
] @punctuation
"#;

const JSON_HIGHLIGHTS: &str = r#"
(string) @string
(number) @number
[(true) (false) (null)] @number
[
  "{" "}" "[" "]" ":" ","
] @punctuation
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_keywords_and_strings() {
        let reg = SyntaxRegistry::new();
        let spans = reg.highlight("rust", "fn main() {\n    let x = \"hi\"; // c\n}\n");
        let has = |scope: Scope, text: &str, src: &str| {
            spans.iter().any(|(r, s)| *s == scope && &src[r.clone()] == text)
        };
        let src = "fn main() {\n    let x = \"hi\"; // c\n}\n";
        assert!(has(Scope::Keyword, "fn", src));
        assert!(has(Scope::Keyword, "let", src));
        assert!(has(Scope::String, "\"hi\"", src));
        assert!(has(Scope::Comment, "// c", src));
        assert!(has(Scope::Function, "main", src));
    }

    #[test]
    fn unknown_language_empty() {
        let reg = SyntaxRegistry::new();
        assert!(reg.highlight("cobol", "hello").is_empty());
    }
}
