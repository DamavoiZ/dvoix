//! Markdown + YAML frontmatter parsing.
//!
//! Frontmatter carries structured metadata (status, tags, due dates, ...) which
//! is what project management and tracking features will query later.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedNote {
    /// Frontmatter as JSON (an empty object if there is none or it is invalid).
    pub frontmatter: Value,
    /// Set when the frontmatter block exists but is not valid YAML.
    pub frontmatter_error: Option<String>,
    /// Title: `title` from frontmatter, else the first `# heading`, else the file name.
    pub title: String,
    /// Tags from frontmatter `tags:` plus inline `#tags`, lower-cased and de-duplicated.
    pub tags: Vec<String>,
    /// Link targets: `[[wikilinks]]` (without alias/heading) and relative markdown links.
    pub links: Vec<String>,
    pub headings: Vec<Heading>,
    /// Plain text of the body, used for full-text search.
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
}

/// Splits `---\n yaml \n---\n body` into (yaml, body). Returns `None` for the
/// yaml part if the document does not start with a frontmatter block.
pub fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let rest = match content.strip_prefix("---\n").or_else(|| content.strip_prefix("---\r\n")) {
        Some(r) => r,
        None => return (None, content),
    };
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return (Some(yaml), body);
        }
        offset += line.len();
    }
    (None, content)
}

pub fn parse(content: &str, file_stem: &str) -> ParsedNote {
    let (yaml, body) = split_frontmatter(content);
    let mut note = ParsedNote { frontmatter: Value::Object(Default::default()), ..Default::default() };

    if let Some(yaml) = yaml {
        if !yaml.trim().is_empty() {
            match serde_yaml_ng::from_str::<Value>(yaml) {
                Ok(Value::Object(map)) => note.frontmatter = Value::Object(map),
                Ok(_) => note.frontmatter_error = Some("frontmatter must be a key/value map".into()),
                Err(e) => note.frontmatter_error = Some(e.to_string()),
            }
        }
    }

    let mut tags: Vec<String> = match note.frontmatter.get("tags") {
        Some(Value::Array(items)) => items.iter().filter_map(|v| v.as_str()).map(normalize_tag).collect(),
        Some(Value::String(s)) => s.split([',', ' ']).filter(|t| !t.is_empty()).map(normalize_tag).collect(),
        _ => Vec::new(),
    };
    let mut links = Vec::new();

    let mut text = String::new();
    // pulldown-cmark splits text at `[`, `#` etc., so we collect adjacent text
    // events into one buffer before scanning for wikilinks and tags.
    let mut buffer = String::new();
    let mut heading: Option<(u8, String)> = None;
    let mut in_code = false;

    let flush = |buffer: &mut String, tags: &mut Vec<String>, links: &mut Vec<String>| {
        scan_inline(buffer, tags, links);
        buffer.clear();
    };

    for event in Parser::new_ext(body, Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Text(t) => {
                text.push_str(&t);
                if let Some((_, h)) = heading.as_mut() {
                    h.push_str(&t);
                }
                if !in_code {
                    buffer.push_str(&t);
                }
            }
            Event::Code(t) => {
                flush(&mut buffer, &mut tags, &mut links);
                text.push_str(&t);
                if let Some((_, h)) = heading.as_mut() {
                    h.push_str(&t);
                }
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut buffer, &mut tags, &mut links);
                in_code = true;
            }
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut buffer, &mut tags, &mut links);
                heading = Some((heading_level(level), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                flush(&mut buffer, &mut tags, &mut links);
                if let Some((level, h)) = heading.take() {
                    note.headings.push(Heading { level, text: h.trim().to_string() });
                }
                text.push('\n');
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                flush(&mut buffer, &mut tags, &mut links);
                if let Some(target) = local_link(&dest_url) {
                    links.push(target);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                text.push('\n');
                buffer.push('\n');
            }
            Event::End(TagEnd::Paragraph | TagEnd::Item) => {
                flush(&mut buffer, &mut tags, &mut links);
                text.push('\n');
            }
            _ => {}
        }
    }
    flush(&mut buffer, &mut tags, &mut links);

    note.title = note
        .frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| note.headings.iter().find(|h| h.level == 1).map(|h| h.text.clone()))
        .unwrap_or_else(|| file_stem.to_string());

    dedup(&mut tags);
    dedup(&mut links);
    note.tags = tags;
    note.links = links;
    note.text = text;
    note
}

fn heading_level(level: HeadingLevel) -> u8 {
    level as u8
}

fn normalize_tag(tag: &str) -> String {
    tag.trim().trim_start_matches('#').to_lowercase()
}

fn dedup(items: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| !i.is_empty() && seen.insert(i.clone()));
}

/// Markdown links to other files in the vault (no URLs, no anchors).
fn local_link(dest: &str) -> Option<String> {
    if dest.is_empty() || dest.starts_with('#') || dest.contains("://") || dest.starts_with("mailto:") {
        return None;
    }
    let target = dest.split('#').next().unwrap_or(dest);
    Some(target.replace("%20", " "))
}

/// Finds `[[target|alias]]` wikilinks and `#tags` in a run of plain text.
fn scan_inline(text: &str, tags: &mut Vec<String>, links: &mut Vec<String>) {
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        match after.find("]]") {
            Some(end) => {
                let inner = &after[..end];
                let target = inner.split(['|', '#']).next().unwrap_or("").trim();
                if !target.is_empty() {
                    links.push(target.to_string());
                }
                rest = &after[end + 2..];
            }
            None => break,
        }
    }

    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let at_word_start = i == 0 || chars[i - 1].is_whitespace() || chars[i - 1] == '(';
        if chars[i] == '#' && at_word_start {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | '/')) {
                j += 1;
            }
            let tag: String = chars[i + 1..j].iter().collect();
            // A tag needs at least one non-digit so "#1" (issue numbers) is not a tag.
            if tag.chars().any(|c| !c.is_ascii_digit()) {
                tags.push(tag.to_lowercase());
            }
            i = j;
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntitle: Projekt Alpha\nstatus: active\ntags: [Projekt, work]\ndue: 2026-12-01\n---\n# Überschrift\n\nSiehe [[Beta|die Beta]] und [Gamma](gamma.md#ziele), aber nicht [Web](https://example.com).\n\nEin #idee Tag und #2 ist keiner.\n\n```\n#kein-tag [[kein-link]]\n```\n";

    #[test]
    fn parses_frontmatter_links_and_tags() {
        let n = parse(NOTE, "alpha");
        assert_eq!(n.title, "Projekt Alpha");
        assert_eq!(n.frontmatter["status"], "active");
        assert_eq!(n.frontmatter["due"], "2026-12-01");
        assert_eq!(n.tags, vec!["projekt", "work", "idee"]);
        assert_eq!(n.links, vec!["Beta", "gamma.md"]);
        assert_eq!(n.headings, vec![Heading { level: 1, text: "Überschrift".into() }]);
        assert!(n.text.contains("Siehe"));
    }

    #[test]
    fn title_falls_back_to_heading_then_file_name() {
        assert_eq!(parse("# Hallo\nText", "datei").title, "Hallo");
        assert_eq!(parse("Nur Text", "datei").title, "datei");
    }

    #[test]
    fn invalid_frontmatter_is_reported_not_fatal() {
        let n = parse("---\n: : :\n---\nbody", "x");
        assert!(n.frontmatter_error.is_some());
        assert!(n.frontmatter.as_object().unwrap().is_empty());
    }

    #[test]
    fn no_frontmatter_without_closing_fence() {
        let (yaml, body) = split_frontmatter("---\nnot closed");
        assert!(yaml.is_none());
        assert_eq!(body, "---\nnot closed");
    }
}
