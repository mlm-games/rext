use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

pub struct StringMatchCandidate {
    pub id: usize,
    pub string: String,
    bag: CharBag,
}

impl StringMatchCandidate {
    pub fn new(id: usize, string: String) -> Self {
        let bag = CharBag::of(&string);
        Self { id, string, bag }
    }
}

#[derive(Clone, Debug)]
pub struct StringMatch {
    pub candidate_id: usize,
    pub score: u32,
    pub positions: Vec<usize>,
}

#[derive(Clone, Copy, Default)]
struct CharBag(u64);

impl CharBag {
    fn of(s: &str) -> Self {
        let mut bag = 0u64;
        for c in s.chars().flat_map(|c| c.to_lowercase()) {
            bag |= 1 << ((c as u32 % 64) as u64);
        }
        Self(bag)
    }

    fn contains(&self, other: CharBag) -> bool {
        self.0 & other.0 == other.0
    }
}

pub fn match_strings(
    query: &str,
    candidates: &[StringMatchCandidate],
    smart_case: bool,
) -> Vec<StringMatch> {
    if query.chars().all(char::is_whitespace) {
        return candidates
            .iter()
            .map(|c| StringMatch {
                candidate_id: c.id,
                score: 0,
                positions: Vec::new(),
            })
            .collect();
    }
    let case = if smart_case && query.chars().any(|c| c.is_uppercase()) {
        CaseMatching::Smart
    } else {
        CaseMatching::Ignore
    };
    let pattern = Pattern::parse(query, case, Normalization::Smart);
    let query_bag = CharBag::of(query);
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    for cand in candidates {
        if !cand.bag.contains(query_bag) {
            continue;
        }
        buf.clear();
        let hay = Utf32Str::new(&cand.string, &mut buf);
        if let Some(score) = pattern.score(hay, &mut matcher) {
            let mut idx = Vec::new();
            pattern.indices(hay, &mut matcher, &mut idx);
            out.push(StringMatch {
                candidate_id: cand.id,
                score,
                positions: idx.into_iter().map(|i| i as usize).collect(),
            });
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.candidate_id.cmp(&b.candidate_id)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands(items: &[&str]) -> Vec<StringMatchCandidate> {
        items
            .iter()
            .enumerate()
            .map(|(i, s)| StringMatchCandidate::new(i, s.to_string()))
            .collect()
    }

    #[test]
    fn ranks_prefix_first() {
        let cs = cands(&["File: Save", "Edit: Select All", "File: Save As"]);
        let m = match_strings("save", &cs, true);
        assert!(m.len() >= 2);
        assert_eq!(m[0].candidate_id, 0);
    }

    #[test]
    fn empty_query_returns_all() {
        let cs = cands(&["a", "b"]);
        assert_eq!(match_strings("", &cs, true).len(), 2);
        assert_eq!(match_strings("   ", &cs, true).len(), 2);
    }

    #[test]
    fn no_match_filtered() {
        let cs = cands(&["alpha", "beta"]);
        assert!(match_strings("zzz", &cs, true).is_empty());
    }

    #[test]
    fn char_bag_prefilter() {
        let cs = cands(&["aaaa", "bbbb"]);
        let m = match_strings("ab", &cs, true);
        assert!(m.is_empty());
    }
}
