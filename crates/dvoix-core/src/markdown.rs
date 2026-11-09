//! Markdown + YAML frontmatter parsing.
//!
//! Frontmatter carries structured metadata (status, tags, due dates, ...) which
//! is what project management and tracking features will query later.

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
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
    let rest = match content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
    {
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
    let mut note = ParsedNote {
        frontmatter: Value::Object(Default::default()),
        ..Default::default()
    };

    if let Some(yaml) = yaml
        && !yaml.trim().is_empty()
    {
        match serde_yaml_ng::from_str::<Value>(yaml) {
            Ok(Value::Object(map)) => note.frontmatter = Value::Object(map),
            Ok(_) => note.frontmatter_error = Some("frontmatter must be a key/value map".into()),
            Err(e) => note.frontmatter_error = Some(e.to_string()),
        }
    }

    let mut tags: Vec<String> = match note.frontmatter.get("tags") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(normalize_tag)
            .collect(),
        Some(Value::String(s)) => s
            .split([',', ' '])
            .filter(|t| !t.is_empty())
            .map(normalize_tag)
            .collect(),
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

    for event in Parser::new_ext(
        body,
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
    ) {
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
                    note.headings.push(Heading {
                        level,
                        text: h.trim().to_string(),
                    });
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
        .or_else(|| {
            note.headings
                .iter()
                .find(|h| h.level == 1)
                .map(|h| h.text.clone())
        })
        .unwrap_or_else(|| file_stem.to_string());

    dedup(&mut tags);
    dedup(&mut links);
    note.tags = tags;
    note.links = links;
    note.text = text;
    note
}

/// A note rendered for the reading view.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedNote {
    /// The body as HTML, without the frontmatter.
    pub html: String,
    /// Frontmatter as JSON, shown as properties.
    pub frontmatter: Value,
}

/// Renders a note for the reading view: the body as HTML and the frontmatter
/// as JSON.
pub fn render(content: &str) -> RenderedNote {
    let (yaml, body) = split_frontmatter(content);
    let mut frontmatter = Value::Object(Default::default());
    if let Some(yaml) = yaml
        && !yaml.trim().is_empty()
        && let Ok(Value::Object(map)) = serde_yaml_ng::from_str::<Value>(yaml)
    {
        frontmatter = Value::Object(map);
    }
    RenderedNote {
        html: render_body(body),
        frontmatter,
    }
}

/// Renders the body of a note as HTML.
///
/// Wikilinks become links with `data-wikilink` so the app can open the target,
/// and HTML inside a note is escaped: a note is data, never code.
pub fn render_body(body: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let mut events: Vec<Event> = Vec::new();
    // pulldown-cmark splits text at `[`, `#` and friends, so a wikilink can be
    // spread over several text events. They are collected before scanning.
    let mut buffer = String::new();

    for event in Parser::new_ext(body, options) {
        match event {
            Event::Text(text) => buffer.push_str(&text),
            other => {
                flush_text(&mut events, &mut buffer);
                match other {
                    // Raw HTML from a note is shown as text. `push_html` escapes
                    // it, so nothing from a note can run in the app.
                    Event::Html(html) | Event::InlineHtml(html) => {
                        events.push(Event::Text(CowStr::from(html.into_string())));
                    }
                    other => events.push(other),
                }
            }
        }
    }
    flush_text(&mut events, &mut buffer);

    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events.into_iter());
    html
}

/// Emits the collected text, turning `[[wikilinks]]` into links.
fn flush_text(events: &mut Vec<Event>, buffer: &mut String) {
    if buffer.is_empty() {
        return;
    }
    let mut rest = buffer.as_str();
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let inner = &after[..end];
        if inner.contains('\n') {
            break;
        }
        if !rest[..start].is_empty() {
            events.push(Event::Text(CowStr::from(rest[..start].to_string())));
        }
        events.push(Event::Html(CowStr::from(wikilink_html(inner))));
        rest = &after[end + 2..];
    }
    if !rest.is_empty() {
        events.push(Event::Text(CowStr::from(rest.to_string())));
    }
    buffer.clear();
}

/// The HTML of a `[[wikilink]]`: the target is in `data-wikilink`, the text is
/// the alias or the target itself.
fn wikilink_html(inner: &str) -> String {
    let (target, alias) = match inner.split_once('|') {
        Some((target, alias)) => (target, Some(alias)),
        None => (inner, None),
    };
    // A heading or block reference belongs to the target, not to the name.
    let target = target.split('#').next().unwrap_or("").trim();
    let label = match alias {
        Some(alias) if !alias.trim().is_empty() => alias.trim(),
        _ => target,
    };
    format!(
        "<a class=\"wikilink\" href=\"#\" data-wikilink=\"{}\">{}</a>",
        escape_html(target),
        escape_html(label)
    )
}

/// Escapes text that is shown as text (raw HTML from a note, link targets).
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
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
    if dest.is_empty()
        || dest.starts_with('#')
        || dest.contains("://")
        || dest.starts_with("mailto:")
    {
        return None;
    }
    let target = dest.split('#').next().unwrap_or(dest);
    Some(target.replace("%20", " "))
}

/// Finds `[[target|alias]]` wikilinks and `#tags` in a run of plain text.
pub(crate) fn scan_inline(text: &str, tags: &mut Vec<String>, links: &mut Vec<String>) {
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
            while j < chars.len()
                && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | '/'))
            {
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
        assert_eq!(
            n.headings,
            vec![Heading {
                level: 1,
                text: "Überschrift".into()
            }]
        );
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

    #[test]
    fn renders_wikilinks_as_links() {
        let html = render_body("Siehe [[Beta]] und [[Gamma|die Gamma]].");
        assert!(html.contains(r##"<a class="wikilink" href="#" data-wikilink="Beta">Beta</a>"##));
        assert!(html.contains(r#"data-wikilink="Gamma">die Gamma</a>"#));
    }

    #[test]
    fn wikilink_target_keeps_the_note_without_its_heading() {
        let html = render_body("[[Beta#Ziele]]");
        assert!(html.contains(r#"data-wikilink="Beta""#));
        assert!(!html.contains("Ziele"));
    }

    #[test]
    fn does_not_link_wikilinks_in_code() {
        let html = render_body("`[[Beta]]`");
        assert!(!html.contains("wikilink"));
    }

    #[test]
    fn escapes_html_from_the_note() {
        let html = render_body("<script>alert(1)</script>");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn renders_tables_tasks_and_strikethrough() {
        let html = render_body("| a | b |\n| --- | --- |\n| 1 | 2 |\n\n- [x] fertig\n\n~~weg~~");
        assert!(html.contains("<table>"));
        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("<del>weg</del>"));
    }

    #[test]
    fn renders_frontmatter_as_properties_and_not_as_content() {
        let note = render("---\ntitle: Projekt Alpha\nstatus: active\n---\n# Hallo");
        assert_eq!(note.frontmatter["title"], "Projekt Alpha");
        assert_eq!(note.frontmatter["status"], "active");
        assert!(note.html.contains("<h1>Hallo</h1>"));
        assert!(!note.html.contains("status"));
    }
}
