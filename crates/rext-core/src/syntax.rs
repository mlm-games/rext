use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Mutex;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(RustEmbed)]
#[folder = "grammars/"]
struct GrammarDir;

pub fn load_queries(name: &str) -> &'static str {
    let path = format!("{name}/highlights.scm");
    let file = GrammarDir::get(&path)
        .unwrap_or_else(|| panic!("missing highlights.scm for language {name:?}"));
    let text = std::str::from_utf8(&file.data)
        .unwrap_or_else(|_| panic!("highlights.scm for {name:?} is not UTF-8"));
    Box::leak(text.to_string().into_boxed_str())
}

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
        languages.insert("python", Language {
            config: python_config(),
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
            "py" | "pyw" | "pyi" => Some("python"),
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
        load_queries("rust"),
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
        load_queries("json"),
        "",
        "",
    )
    .expect("json highlights query");
    config.configure(HIGHLIGHT_NAMES);
    config
}

fn python_config() -> HighlightConfiguration {
    let mut config = HighlightConfiguration::new(
        tree_sitter_python::LANGUAGE.into(),
        "python",
        load_queries("python"),
        "",
        "",
    )
    .expect("python highlights query");
    config.configure(HIGHLIGHT_NAMES);
    config
}

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

    #[test]
    fn python_basics() {
        let reg = SyntaxRegistry::new();
        let src = "import os\n\ndef greet(name):\n    # hi\n    return f\"hi {name}\"\n";
        let spans = reg.highlight("python", src);
        let has = |scope: Scope, text: &str| {
            spans.iter().any(|(r, s)| *s == scope && &src[r.clone()] == text)
        };
        assert!(has(Scope::Keyword, "import"));
        assert!(has(Scope::Keyword, "def"));
        assert!(has(Scope::Keyword, "return"));
        assert!(has(Scope::Function, "greet"));
        assert!(has(Scope::Comment, "# hi"));
    }
}
