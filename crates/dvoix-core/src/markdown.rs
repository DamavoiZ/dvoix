//! Markdown + YAML frontmatter parsing.
//!
//! Frontmatter carries structured metadata (status, tags, due dates, ...) which
//! is what project management and tracking features will query later.

use std::borrow::Cow;

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
                    links.push(target.into_owned());
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
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let mut events: Vec<Event> = Vec::new();
    // pulldown-cmark splits text at `[`, `#` and friends, so a wikilink can be
    // spread over several text events. They are collected before scanning.
    let mut buffer = String::new();
    // Inside a code block `[[x]]` is code, not a link, and the reading view
    // highlights the block from its plain text.
    let mut in_code = false;

    for event in Parser::new_ext(body, options) {
        match event {
            Event::Text(text) if in_code => events.push(Event::Text(text)),
            Event::Text(text) => buffer.push_str(&text),
            other => {
                flush_text(&mut events, &mut buffer);
                match &other {
                    Event::Start(Tag::CodeBlock(_)) => in_code = true,
                    Event::End(TagEnd::CodeBlock) => in_code = false,
                    _ => {}
                }
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
        // `![[file]]` embeds the file: the `!` is part of the link, not text.
        let embed = rest[..start].ends_with('!');
        let before = if embed {
            &rest[..start - 1]
        } else {
            &rest[..start]
        };
        if !before.is_empty() {
            events.push(Event::Text(CowStr::from(before.to_string())));
        }
        events.push(Event::Html(CowStr::from(wikilink_html(inner, embed))));
        rest = &after[end + 2..];
    }
    if !rest.is_empty() {
        events.push(Event::Text(CowStr::from(rest.to_string())));
    }
    buffer.clear();
}

/// The HTML of a `[[wikilink]]`: the target is in `data-wikilink`, the text is
/// the alias or the target itself.
///
/// An embed (`![[image.png]]`) gets `data-embed` as well; the reading view
/// shows images in its place and keeps the link for every other file.
fn wikilink_html(inner: &str, embed: bool) -> String {
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
    let target = escape_html(target);
    let label = escape_html(label);
    if embed {
        // In `![[image.png|300]]` the part after the pipe is the width.
        format!(
            "<a class=\"wikilink embed\" href=\"#\" data-wikilink=\"{target}\" data-embed=\"{target}\">{label}</a>"
        )
    } else {
        format!("<a class=\"wikilink\" href=\"#\" data-wikilink=\"{target}\">{label}</a>")
    }
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
    // The set borrows the strings instead of cloning every one of them; a
    // note can repeat the same tag or link many times.
    let mut seen = std::collections::HashSet::with_capacity(items.len());
    let keep: Vec<bool> = items
        .iter()
        .map(|i| !i.is_empty() && seen.insert(i.as_str()))
        .collect();
    let mut keep = keep.into_iter();
    items.retain(|_| keep.next() == Some(true));
}

/// Markdown links to other files in the vault (no URLs, no anchors).
fn local_link(dest: &str) -> Option<Cow<'_, str>> {
    if dest.is_empty()
        || dest.starts_with('#')
        || dest.contains("://")
        || dest.starts_with("mailto:")
    {
        return None;
    }
    let target = dest.split('#').next().unwrap_or(dest);
    // Most targets have no escaped space; then no copy is needed.
    Some(if target.contains("%20") {
        Cow::Owned(target.replace("%20", " "))
    } else {
        Cow::Borrowed(target)
    })
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

    // Walk the text once without collecting it into a `Vec<char>` first;
    // this runs for every paragraph of every note that is indexed.
    let mut chars = text.char_indices().peekable();
    let mut prev: Option<char> = None;
    while let Some((i, c)) = chars.next() {
        let at_word_start = prev.is_none_or(|p| p.is_whitespace() || p == '(');
        if c == '#' && at_word_start {
            let start = i + 1;
            let mut end = start;
            prev = Some('#');
            while let Some(&(j, tc)) = chars.peek() {
                if !(tc.is_alphanumeric() || matches!(tc, '_' | '-' | '/')) {
                    break;
                }
                end = j + tc.len_utf8();
                prev = Some(tc);
                chars.next();
            }
            let tag = &text[start..end];
            // A tag needs at least one non-digit so "#1" (issue numbers) is not a tag.
            if tag.chars().any(|c| !c.is_ascii_digit()) {
                tags.push(tag.to_lowercase());
            }
        } else {
            prev = Some(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntitle: Project Alpha\nstatus: active\ntags: [Project, work]\ndue: 2026-12-01\n---\n# Heading\n\nSee [[Beta|the Beta]] and [Gamma](gamma.md#goals), but not [Web](https://example.com).\n\nAn #idea tag and #2 is not one.\n\n```\n#no-tag [[no-link]]\n```\n";

    #[test]
    fn parses_frontmatter_links_and_tags() {
        let n = parse(NOTE, "alpha");
        assert_eq!(n.title, "Project Alpha");
        assert_eq!(n.frontmatter["status"], "active");
        assert_eq!(n.frontmatter["due"], "2026-12-01");
        assert_eq!(n.tags, vec!["project", "work", "idea"]);
        assert_eq!(n.links, vec!["Beta", "gamma.md"]);
        assert_eq!(
            n.headings,
            vec![Heading {
                level: 1,
                text: "Heading".into()
            }]
        );
        assert!(n.text.contains("See"));
    }

    #[test]
    fn title_falls_back_to_heading_then_file_name() {
        assert_eq!(parse("# Hello\nText", "file").title, "Hello");
        assert_eq!(parse("Only text", "file").title, "file");
    }

    #[test]
    fn scans_tags_and_wikilinks_in_plain_text() {
        let mut tags = Vec::new();
        let mut links = Vec::new();
        scan_inline(
            "#start (#paren) x#no ##no #12 #ok-1/sub #Ünicode [[Note|alias]] [[Other#head]] [[ ]]",
            &mut tags,
            &mut links,
        );
        assert_eq!(tags, ["start", "paren", "ok-1/sub", "ünicode"]);
        assert_eq!(links, ["Note", "Other"]);

        // An empty run is scanned for every flush; it must stay cheap and quiet.
        let mut tags = Vec::new();
        let mut links = Vec::new();
        scan_inline("", &mut tags, &mut links);
        assert!(tags.is_empty() && links.is_empty());
    }

    #[test]
    fn invalid_frontmatter_is_reported_not_fatal() {
        let n = parse("---\n: : :\n---\nbody", "x");
        assert!(n.frontmatter_error.is_some());
        assert!(n.frontmatter.as_object().unwrap().is_empty());
    }

    /// A tag runs to the next character that does not belong to it; what
    /// stands right after a tag must not start a new one.
    #[test]
    fn tags_end_at_the_first_foreign_character() {
        let mut tags = Vec::new();
        let mut links = Vec::new();
        scan_inline("#one#two (#three)#four #five-", &mut tags, &mut links);
        assert_eq!(tags, ["one", "three", "five-"]);
    }

    /// Repeated tags and links are kept once, in the order of their first
    /// appearance; empty items go away.
    #[test]
    fn parse_dedups_tags_and_links_in_order() {
        let n = parse(
            "#work #idea #work [[Beta]] #idea [[Beta]] [[Alpha]]",
            "file",
        );
        assert_eq!(n.tags, ["work", "idea"]);
        assert_eq!(n.links, ["Beta", "Alpha"]);
    }

    /// A markdown link target keeps its spaces: `%20` becomes a space, and
    /// a target without it is taken as it is.
    #[test]
    fn markdown_link_targets_unescape_spaces() {
        let n = parse(
            "[A](my%20note.md) [B](plain.md) [C](with%20space.md#head) [D](https://x.dev/a%20b)",
            "file",
        );
        assert_eq!(n.links, ["my note.md", "plain.md", "with space.md"]);
    }

    #[test]
    fn local_links_skip_urls_and_anchors() {
        assert_eq!(local_link(""), None);
        assert_eq!(local_link("#head"), None);
        assert_eq!(local_link("mailto:a@b.c"), None);
        assert_eq!(local_link("plain.md").as_deref(), Some("plain.md"));
        assert!(matches!(local_link("plain.md"), Some(Cow::Borrowed(_))));
        assert_eq!(
            local_link("a%20b.md").as_deref(),
            Some("a b.md")
        );
    }

    #[test]
    fn no_frontmatter_without_closing_fence() {
        let (yaml, body) = split_frontmatter("---\nnot closed");
        assert!(yaml.is_none());
        assert_eq!(body, "---\nnot closed");
    }

    #[test]
    fn renders_wikilinks_as_links() {
        let html = render_body("See [[Beta]] and [[Gamma|the Gamma]].");
        assert!(html.contains(r##"<a class="wikilink" href="#" data-wikilink="Beta">Beta</a>"##));
        assert!(html.contains(r#"data-wikilink="Gamma">the Gamma</a>"#));
    }

    #[test]
    fn renders_embeds_with_their_target() {
        let html = render_body("Look: ![[Pasted image 1.png]] and [[Beta]]");
        assert!(html.contains(
            r##"Look: <a class="wikilink embed" href="#" data-wikilink="Pasted image 1.png" data-embed="Pasted image 1.png">Pasted image 1.png</a>"##
        ));
        assert!(html.contains(r#"data-wikilink="Beta">Beta</a>"#));
        assert!(!html.contains('!'));
    }

    #[test]
    fn wikilink_target_keeps_the_note_without_its_heading() {
        let html = render_body("[[Beta#goals]]");
        assert!(html.contains(r#"data-wikilink="Beta""#));
        assert!(!html.contains("goals"));
    }

    #[test]
    fn does_not_link_wikilinks_in_code() {
        let html = render_body("`[[Beta]]`");
        assert!(!html.contains("wikilink"));
    }

    #[test]
    fn keeps_code_blocks_as_plain_text_with_their_language() {
        let html = render_body("```rust\nlet a = \"[[Beta]]\";\n```\n");
        assert!(html.contains(r#"<code class="language-rust">"#));
        assert!(!html.contains("wikilink"));
        assert!(html.contains("[[Beta]]"));
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
        let note = render("---\ntitle: Project Alpha\nstatus: active\n---\n# Hello");
        assert_eq!(note.frontmatter["title"], "Project Alpha");
        assert_eq!(note.frontmatter["status"], "active");
        assert!(note.html.contains("<h1>Hello</h1>"));
        assert!(!note.html.contains("status"));
    }
}
