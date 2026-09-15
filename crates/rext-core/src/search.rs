use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_ignored: bool,
    pub max_results: usize,
    pub max_file_size: u64,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            include_ignored: false,
            max_results: 1000,
            max_file_size: 4 * 1024 * 1024,
        }
    }
}

pub enum SearchQuery {
    Text {
        automaton: AhoCorasick,
        needle: String,
        options: SearchOptions,
    },
    Regex {
        regex: regex::Regex,
        options: SearchOptions,
    },
}

impl SearchQuery {
    pub fn text(needle: &str, options: SearchOptions) -> Result<Self, String> {
        if needle.is_empty() {
            return Err("empty search".into());
        }
        let automaton = AhoCorasickBuilder::new()
            .ascii_case_insensitive(!options.case_sensitive)
            .build([needle])
            .map_err(|e| e.to_string())?;
        Ok(SearchQuery::Text {
            automaton,
            needle: needle.to_string(),
            options,
        })
    }

    pub fn regex(pattern: &str, options: SearchOptions) -> Result<Self, String> {
        let mut builder = regex::RegexBuilder::new(pattern);
        builder.case_insensitive(!options.case_sensitive);
        let regex = builder.build().map_err(|e| e.to_string())?;
        Ok(SearchQuery::Regex { regex, options })
    }

    pub fn options(&self) -> &SearchOptions {
        match self {
            SearchQuery::Text { options, .. } => options,
            SearchQuery::Regex { options, .. } => options,
        }
    }

    fn is_word_char(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_'
    }

    fn whole_word_ok(&self, hay: &[u8], m: Range<usize>) -> bool {
        if !self.options().whole_word {
            return true;
        }
        let left_ok = m.start == 0 || !Self::is_word_char(hay[m.start - 1]);
        let right_ok = m.end >= hay.len() || !Self::is_word_char(hay[m.end]);
        left_ok && right_ok
    }

    pub fn find_in_line(&self, line: &str) -> Vec<Range<usize>> {
        let hay = line.as_bytes();
        match self {
            SearchQuery::Text {
                automaton, needle, ..
            } => {
                if needle.contains('\n') {
                    return Vec::new();
                }
                automaton
                    .find_iter(hay)
                    .map(|m| m.start()..m.end())
                    .filter(|m| self.whole_word_ok(hay, m.clone()))
                    .collect()
            }
            SearchQuery::Regex { regex, .. } => regex
                .find_iter(line)
                .map(|m| m.start()..m.end())
                .filter(|m| self.whole_word_ok(hay, m.clone()))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub path: PathBuf,
    pub row: usize,
    pub ranges: Vec<Range<usize>>,
    pub preview: String,
}

fn default_ignores() -> Vec<String> {
    vec![
        ".git".into(),
        "target".into(),
        "node_modules".into(),
        ".hg".into(),
        ".svn".into(),
        "__pycache__".into(),
        "dist".into(),
        "build".into(),
    ]
}

fn is_ignored(path: &Path, root: &Path, include_ignored: bool) -> bool {
    if include_ignored {
        return false;
    }
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let ignores = default_ignores();
    rel.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') && s.len() > 1 && rel.components().count() > 1
            || ignores.iter().any(|i| *s == **i)
            || s.starts_with('.')
    })
}

fn collect_files(root: &Path, include_ignored: bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if is_ignored(&path, root, include_ignored) {
            continue;
        }
        if path.is_dir() {
            // Recurse with the original root for ignore matching.
            collect_files_inner(&path, root, include_ignored, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn collect_files_inner(dir: &Path, root: &Path, include_ignored: bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if is_ignored(&path, root, include_ignored) {
            continue;
        }
        if path.is_dir() {
            collect_files_inner(&path, root, include_ignored, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

pub fn search_project(
    root: &Path,
    query: &SearchQuery,
    open_buffers: &[(PathBuf, String)],
) -> Vec<SearchHit> {
    let opts = query.options();
    let mut files = Vec::new();
    collect_files(root, opts.include_ignored, &mut files);

    let open: std::collections::HashMap<&Path, &str> = open_buffers
        .iter()
        .map(|(p, s)| (p.as_path(), s.as_str()))
        .collect();

    let mut hits = Vec::new();
    for path in files {
        if hits.len() >= opts.max_results {
            break;
        }
        // Open-buffer fast path: search live contents, skip disk.
        if let Some(contents) = open.get(path.as_path()) {
            search_text(&mut hits, path, contents, query, opts);
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.len() > opts.max_file_size {
            continue;
        }
        let Ok(contents) = std::fs::read(&path) else {
            continue;
        };
        if contents.contains(&0) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&contents) else {
            continue;
        };
        search_text(&mut hits, path, text, query, opts);
    }
    hits
}

fn search_text(
    hits: &mut Vec<SearchHit>,
    path: PathBuf,
    text: &str,
    query: &SearchQuery,
    opts: &SearchOptions,
) {
    for (row, line) in text.lines().enumerate() {
        if hits.len() >= opts.max_results {
            return;
        }
        let ranges = query.find_in_line(line);
        if ranges.is_empty() {
            continue;
        }
        hits.push(SearchHit {
            path: path.clone(),
            row,
            ranges,
            preview: line.chars().take(160).collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_case_insensitive() {
        let q = SearchQuery::text("hello", SearchOptions::default()).unwrap();
        assert_eq!(q.find_in_line("say HELLO now"), vec![4..9]);
        assert!(q.find_in_line("nothing").is_empty());
    }

    #[test]
    fn text_case_sensitive() {
        let q = SearchQuery::text(
            "hello",
            SearchOptions {
                case_sensitive: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(q.find_in_line("say HELLO now").is_empty());
        assert_eq!(q.find_in_line("say hello now"), vec![4..9]);
    }

    #[test]
    fn whole_word() {
        let q = SearchQuery::text(
            "he",
            SearchOptions {
                whole_word: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(q.find_in_line("hello").is_empty());
        assert_eq!(q.find_in_line("say he now"), vec![4..6]);
    }

    #[test]
    fn regex_query() {
        let q = SearchQuery::regex(r"\bfn\s+(\w+)", SearchOptions::default()).unwrap();
        assert_eq!(q.find_in_line("fn main() {"), vec![0..7]);
    }

    #[test]
    fn regex_invalid() {
        assert!(SearchQuery::regex("(", SearchOptions::default()).is_err());
    }

    #[test]
    fn project_search_prefers_open_buffers() {
        let dir = std::env::temp_dir().join("rext_search_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "old content").unwrap();
        let q = SearchQuery::text("new", SearchOptions::default()).unwrap();
        // Disk has "old", open buffer has "new" → hit comes from buffer.
        let hits = search_project(
            &dir,
            &q,
            &[(dir.join("a.txt"), "new content".into())],
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].row, 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ignores_target_and_hidden() {
        let dir = std::env::temp_dir().join("rext_search_ignore");
        std::fs::create_dir_all(dir.join("target")).unwrap();
        std::fs::create_dir_all(dir.join(".hidden")).unwrap();
        std::fs::write(dir.join("target").join("x.txt"), "needle").unwrap();
        std::fs::write(dir.join(".hidden").join("y.txt"), "needle").unwrap();
        std::fs::write(dir.join("z.txt"), "needle").unwrap();
        let q = SearchQuery::text("needle", SearchOptions::default()).unwrap();
        let hits = search_project(&dir, &q, &[]);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("z.txt"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
