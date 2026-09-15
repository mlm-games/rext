use smallvec::SmallVec;
use std::{collections::BTreeMap, ops::Range};
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snippet {
    pub text: String,
    pub tabstops: Vec<TabStop>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabStop {
    pub ranges: SmallVec<[Range<isize>; 2]>,
    pub choices: Option<Vec<String>>,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SnippetError {
    #[error("expected an integer")]
    ExpectedInteger,
    #[error("expected a closing brace")]
    ExpectedClosingBrace,
    #[error("placeholder choice doesn't contain closing pipe '|'")]
    UnterminatedChoice,
    #[error("invalid integer: {0}")]
    InvalidInteger(String),
}

impl Snippet {
    pub fn parse(source: &str) -> Result<Self, SnippetError> {
        let mut text = String::with_capacity(source.len());
        let mut tabstops = BTreeMap::new();
        parse_snippet(source, false, &mut text, &mut tabstops)?;

        let len = text.len() as isize;
        let final_tabstop = tabstops.remove(&0);
        let mut tabstops = tabstops.into_values().collect::<Vec<_>>();

        if let Some(final_tabstop) = final_tabstop {
            tabstops.push(final_tabstop);
        } else {
            let end_tabstop = TabStop {
                ranges: [len..len].into_iter().collect(),
                choices: None,
            };
            if !tabstops.last().is_some_and(|t| *t == end_tabstop) {
                tabstops.push(end_tabstop);
            }
        }

        Ok(Snippet { text, tabstops })
    }
}

fn parse_snippet<'a>(
    mut source: &'a str,
    nested: bool,
    text: &mut String,
    tabstops: &mut BTreeMap<usize, TabStop>,
) -> Result<&'a str, SnippetError> {
    loop {
        match source.chars().next() {
            None => return Ok(""),
            Some('$') => {
                source = parse_tabstop(&source[1..], text, tabstops)?;
            }
            Some('\\') => {
                source = &source[1..];
                if let Some(c) = source.chars().next() {
                    if c == '$' || c == '\\' || c == '}' {
                        text.push(c);
                        source = &source[1..];
                    } else {
                        text.push('\\');
                    }
                } else {
                    text.push('\\');
                }
            }
            Some('}') => {
                if nested {
                    return Ok(source);
                } else {
                    text.push('}');
                    source = &source[1..];
                }
            }
            Some(_) => {
                let chunk_end = source.find(['}', '$', '\\']).unwrap_or(source.len());
                let (chunk, rest) = source.split_at(chunk_end);
                text.push_str(chunk);
                source = rest;
            }
        }
    }
}

fn parse_tabstop<'a>(
    mut source: &'a str,
    text: &mut String,
    tabstops: &mut BTreeMap<usize, TabStop>,
) -> Result<&'a str, SnippetError> {
    let tabstop_start = text.len();
    let tabstop_index;
    let mut choices = None;

    if let Some(rest) = source.strip_prefix('{') {
        let (index, rest) = parse_int(&rest[0..])?;
        tabstop_index = index;
        source = rest;

        if let Some(rest) = source.strip_prefix('|') {
            let (r, c) = parse_choices(&rest[0..], text)?;
            source = r;
            choices = c;
        }

        if let Some(rest) = source.strip_prefix(':') {
            source = parse_snippet(&rest[0..], true, text, tabstops)?;
        }

        if let Some(rest) = source.strip_prefix('}') {
            source = rest;
        } else {
            return Err(SnippetError::ExpectedClosingBrace);
        }
    } else {
        let (index, rest) = parse_int(source)?;
        tabstop_index = index;
        source = rest;
    }

    tabstops
        .entry(tabstop_index)
        .or_insert_with(|| TabStop {
            ranges: Default::default(),
            choices,
        })
        .ranges
        .push(tabstop_start as isize..text.len() as isize);
    Ok(source)
}

fn parse_int(source: &str) -> Result<(usize, &str), SnippetError> {
    let len = source
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(source.len());
    if len == 0 {
        return Err(SnippetError::ExpectedInteger);
    }
    let (prefix, suffix) = source.split_at(len);
    prefix
        .parse()
        .map(|n| (n, suffix))
        .map_err(|_| SnippetError::InvalidInteger(prefix.to_string()))
}

fn parse_choices<'a>(
    mut source: &'a str,
    text: &mut String,
) -> Result<(&'a str, Option<Vec<String>>), SnippetError> {
    let mut found_default_choice = false;
    let mut current_choice = String::new();
    let mut choices = Vec::new();

    loop {
        match source.chars().next() {
            None => return Ok(("", Some(choices))),
            Some('\\') => {
                source = &source[1..];
                if let Some(c) = source.chars().next() {
                    current_choice.push(c);
                    if !found_default_choice {
                        text.push(c);
                    }
                    source = &source[c.len_utf8()..];
                }
            }
            Some(',') => {
                found_default_choice = true;
                source = &source[1..];
                choices.push(current_choice);
                current_choice = String::new();
            }
            Some('|') => {
                source = &source[1..];
                choices.push(current_choice);
                return Ok((source, Some(choices)));
            }
            Some(_) => {
                let Some(chunk_end) = source.find([',', '|', '\\']) else {
                    return Err(SnippetError::UnterminatedChoice);
                };
                let (chunk, rest) = source.split_at(chunk_end);
                if !found_default_choice {
                    text.push_str(chunk);
                }
                current_choice.push_str(chunk);
                source = rest;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabstops(s: &Snippet) -> Vec<Vec<Range<isize>>> {
        s.tabstops.iter().map(|t| t.ranges.to_vec()).collect()
    }

    #[test]
    fn no_tabstops() {
        let s = Snippet::parse("one-two-three").unwrap();
        assert_eq!(s.text, "one-two-three");
        assert_eq!(tabstops(&s), &[vec![13..13]]);
    }

    #[test]
    fn numbered_tabstops() {
        let s = Snippet::parse("one$1two").unwrap();
        assert_eq!(s.text, "onetwo");
        assert_eq!(tabstops(&s), &[vec![3..3], vec![6..6]]);

        let s = Snippet::parse("one$123-$99-two").unwrap();
        assert_eq!(s.text, "one--two");
        assert_eq!(tabstops(&s), &[vec![4..4], vec![3..3], vec![8..8]]);
    }

    #[test]
    fn explicit_final_tabstop() {
        let s = Snippet::parse(r#"<div class="$1">$0</div>"#).unwrap();
        assert_eq!(s.text, r#"<div class=""></div>"#);
        assert_eq!(tabstops(&s), &[vec![12..12], vec![14..14]]);
    }

    #[test]
    fn placeholders() {
        let s = Snippet::parse("one${1:two}three${2:four}").unwrap();
        assert_eq!(s.text, "onetwothreefour");
        assert_eq!(tabstops(&s), &[vec![3..6], vec![11..15], vec![15..15]]);
    }

    #[test]
    fn choices() {
        let s = Snippet::parse("type ${1|i32, u32|} = $2").unwrap();
        assert_eq!(s.text, "type i32 = ");
        assert_eq!(
            s.tabstops[0].choices,
            Some(vec!["i32".into(), " u32".into()])
        );
    }

    #[test]
    fn escapes() {
        let s = Snippet::parse(r"\$1 \} \\").unwrap();
        assert_eq!(s.text, "$1 } \\");
    }

    #[test]
    fn errors() {
        assert_eq!(
            Snippet::parse("a${1:x").unwrap_err(),
            SnippetError::ExpectedClosingBrace
        );
        assert_eq!(
            Snippet::parse("a$").unwrap_err(),
            SnippetError::ExpectedInteger
        );
    }
}
