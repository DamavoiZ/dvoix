//! Tasks are ordinary markdown checkboxes, anywhere in any note:
//!
//! ```markdown
//! - [ ] Write the offer 📅 2026-12-01 ⏫ #client
//! - [x] Done
//! * [ ] Also with `due:2026-12-01` and `!high`
//! ```
//!
//! Optional metadata is written inline so the file stays readable everywhere:
//!
//! | Meaning  | Syntax                                                     |
//! | -------- | ---------------------------------------------------------- |
//! | Due date | `📅 2026-12-01` or `due:2026-12-01`                        |
//! | Priority | `⏫` / `🔼` / `🔽` or `!high` / `!medium` / `!low` (German `!hoch` / `!mittel` / `!niedrig` also work) |
//!
//! The emoji forms match the Obsidian Tasks plugin, so existing vaults keep working.
//! The file is the source of truth: toggling a task rewrites exactly that one line.

use serde::Serialize;

use crate::markdown;
use crate::vault::{Vault, VaultEntry};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    /// Accepts English and German words (`high`, `hoch`, ...) and `1`-`3`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "hoch" | "high" | "1" | "dringend" | "urgent" => Some(Priority::High),
            "mittel" | "medium" | "normal" | "2" => Some(Priority::Medium),
            "niedrig" | "low" | "3" => Some(Priority::Low),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// 0-based line number in the file (frontmatter included).
    pub line: u32,
    pub done: bool,
    /// Task text without the checkbox and without due/priority markers.
    pub text: String,
    /// `YYYY-MM-DD`
    pub due: Option<String>,
    pub priority: Option<Priority>,
    pub tags: Vec<String>,
    /// Nesting depth (leading whitespace, tab = 4), so subtasks can be indented.
    pub indent: u32,
}

/// The parts of a task line: everything before the checkbox char, the char, everything after `]`.
struct Checkbox<'a> {
    marker_char: usize,
    state: char,
    rest: &'a str,
    indent: u32,
}

/// Recognises `- [ ] text`, `* [x] text`, `+ [ ] text` and `1. [ ] text`.
fn checkbox(line: &str) -> Option<Checkbox<'_>> {
    let trimmed = line.trim_start();
    let lead = &line[..line.len() - trimmed.len()];
    let indent = lead.chars().map(|c| if c == '\t' { 4 } else { 1 }).sum();

    let after_bullet = if let Some(r) = trimmed.strip_prefix(['-', '*', '+']) {
        r
    } else {
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 || digits > 9 {
            return None;
        }
        trimmed[digits..].strip_prefix(['.', ')'])?
    };
    let spaces = after_bullet.len() - after_bullet.trim_start_matches([' ', '\t']).len();
    if spaces == 0 {
        return None;
    }
    let boxed = &after_bullet[spaces..];
    let mut chars = boxed.chars();
    if chars.next() != Some('[') {
        return None;
    }
    let state = chars.next()?;
    if !matches!(state, ' ' | 'x' | 'X') || chars.next() != Some(']') {
        return None;
    }
    let rest = &boxed[3..];
    // `- [ ]` needs a space (or end of line) after the box to be a task.
    if !(rest.is_empty() || rest.starts_with([' ', '\t'])) {
        return None;
    }
    let marker_char = line.len() - boxed.len() + 1;
    Some(Checkbox {
        marker_char,
        state,
        rest,
        indent,
    })
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Accepts `2026-12-01`, or a YAML date/datetime that starts with one.
pub fn normalize_date(s: &str) -> Option<String> {
    let s = s.trim();
    let date = s.get(..10)?;
    is_date(date).then(|| date.to_string())
}

/// Pulls due date and priority markers out of the task text.
fn parse_meta(rest: &str) -> (String, Option<String>, Option<Priority>) {
    let mut due = None;
    let mut priority = None;
    let mut words: Vec<&str> = Vec::new();
    let mut tokens = rest.split_whitespace().peekable();
    while let Some(tok) = tokens.next() {
        match tok {
            "📅" | "🗓" | "🗓️" if tokens.peek().is_some_and(|n| is_date(n)) => {
                due = tokens.next().map(str::to_string);
            }
            "⏫" | "🔺" => priority = Some(Priority::High),
            "🔼" => priority = Some(Priority::Medium),
            "🔽" | "⏬" => priority = Some(Priority::Low),
            _ => {
                if let Some(d) = tok.strip_prefix("due:").filter(|d| is_date(d)) {
                    due = Some(d.to_string());
                } else if let Some(p) = tok.strip_prefix('!').and_then(Priority::parse) {
                    priority = Some(p);
                } else if let Some(d) = tok.strip_prefix("📅").filter(|d| is_date(d)) {
                    due = Some(d.to_string());
                } else {
                    words.push(tok);
                }
            }
        }
    }
    (words.join(" "), due, priority)
}

fn fence(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if t.starts_with("```") {
        Some("```")
    } else if t.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

/// All checkbox tasks of a note, skipping frontmatter and fenced code blocks.
pub fn extract(content: &str) -> Vec<Task> {
    let mut tasks = Vec::new();
    for_each_task(content, None, |task| {
        tasks.push(task);
        true
    });
    tasks
}

/// Walks the checkbox tasks of a note, skipping frontmatter and fenced code
/// blocks, and hands each one to `f`. Stops when `f` returns `false` or when
/// the line `until` is reached (that line is still handed over). Lines before
/// `until` are only recognised, not parsed: a toggle needs the metadata of
/// one line, not of every task above it.
fn for_each_task(content: &str, until: Option<u32>, mut f: impl FnMut(Task) -> bool) {
    let mut lines = content.lines().enumerate().peekable();

    // Skip the frontmatter block, if any.
    if matches!(lines.peek(), Some((_, l)) if l.trim_end() == "---")
        && markdown::split_frontmatter(content).0.is_some()
    {
        lines.next();
        for (_, l) in lines.by_ref() {
            if l.trim_end() == "---" {
                break;
            }
        }
    }

    let mut open_fence: Option<&str> = None;
    for (n, line) in lines {
        let n = n as u32;
        if let Some(f) = open_fence {
            if line.trim_start().starts_with(f) {
                open_fence = None;
            }
            if until == Some(n) {
                return; // the target line is inside a code block
            }
            continue;
        }
        if let Some(f) = fence(line) {
            open_fence = Some(f);
            if until == Some(n) {
                return; // the target line opens a code block
            }
            continue;
        }
        let Some(cb) = checkbox(line) else {
            if until == Some(n) {
                return; // the target line is not a task
            }
            continue;
        };
        if until.is_some_and(|u| n < u) {
            continue; // an earlier task: recognised, but not parsed
        }
        let (text, due, priority) = parse_meta(cb.rest);
        let mut tags = Vec::new();
        markdown::scan_inline(&text, &mut tags, &mut Vec::new());
        tags.dedup();
        let reached = until == Some(n);
        if !f(Task {
            line: n,
            done: cb.state != ' ',
            text,
            due,
            priority,
            tags,
            indent: cb.indent,
        }) || reached
        {
            return;
        }
    }
}

/// The task on `line`, or `None` when that line is not a task. Only that
/// line is parsed, so a toggle does not pay for the tasks of the whole note.
fn task_at_line(content: &str, line: u32) -> Option<Task> {
    let mut found = None;
    for_each_task(content, Some(line), |task| {
        found = Some(task);
        false
    });
    found
}

/// Checks or unchecks the task on `line`. Returns the new file content, or
/// `None` if that line is not a task (any more) or its text is not `expected_text`
/// (the file changed since the caller last looked).
pub fn set_done(content: &str, line: u32, expected_text: &str, done: bool) -> Option<String> {
    let task = task_at_line(content, line)?;
    if task.text != expected_text {
        return None;
    }
    let mut out = String::with_capacity(content.len());
    for (n, l) in content.split_inclusive('\n').enumerate() {
        if n as u32 == line {
            let cb = checkbox(l.trim_end_matches(['\n', '\r']))?;
            out.push_str(&l[..cb.marker_char]);
            out.push(if done { 'x' } else { ' ' });
            out.push_str(&l[cb.marker_char + 1..]);
        } else {
            out.push_str(l);
        }
    }
    Some(out)
}

/// Toggles a task directly in its file. The caller re-indexes the file afterwards.
pub fn set_done_in_vault(
    vault: &Vault,
    rel: &str,
    line: u32,
    expected_text: &str,
    done: bool,
) -> Result<VaultEntry> {
    let content = vault.read(rel)?;
    let updated = set_done(&content, line, expected_text, done)
        .ok_or_else(|| Error::StaleTask(rel.to_string()))?;
    vault.write(rel, &updated)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "---\ntitle: X\nlist:\n- [ ] no task in frontmatter\n---\n# List\n\n- [ ] Write the offer 📅 2026-12-01 ⏫ #Client\n  - [x] Subtask\n* [X] Uppercase done\n1. [ ] Numbered due:2026-11-20 !low\n- [] no task\n-[ ] no task\n- [ ]without space\n\n```\n- [ ] in code\n```\n- [ ] End !medium\n";

    #[test]
    fn extracts_tasks_with_metadata() {
        let tasks = extract(NOTE);
        let texts: Vec<_> = tasks.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "Write the offer #Client",
                "Subtask",
                "Uppercase done",
                "Numbered",
                "End"
            ]
        );

        let first = &tasks[0];
        assert_eq!(first.line, 7);
        assert!(!first.done);
        assert_eq!(first.due.as_deref(), Some("2026-12-01"));
        assert_eq!(first.priority, Some(Priority::High));
        assert_eq!(first.tags, ["client"]);

        assert!(tasks[1].done);
        assert_eq!(tasks[1].indent, 2);
        assert!(tasks[2].done);
        assert_eq!(tasks[3].due.as_deref(), Some("2026-11-20"));
        assert_eq!(tasks[3].priority, Some(Priority::Low));
        assert_eq!(tasks[4].priority, Some(Priority::Medium));
    }

    #[test]
    fn toggles_exactly_one_line() {
        let content = "# A\r\n- [ ] one\r\n- [ ] two\r\n";
        let done = set_done(content, 2, "two", true).unwrap();
        assert_eq!(done, "# A\r\n- [ ] one\r\n- [x] two\r\n");
        let undone = set_done(&done, 2, "two", false).unwrap();
        assert_eq!(undone, content);
    }

    #[test]
    fn refuses_stale_toggle() {
        let content = "- [ ] one\n- [ ] two\n";
        assert!(set_done(content, 1, "one", true).is_none());
        assert!(set_done(content, 5, "one", true).is_none());
        assert!(set_done("# no task", 0, "", true).is_none());
    }

    /// A toggle reads the metadata of its own line only: tasks above it are
    /// recognised but not parsed, and a checkbox inside a code block is not
    /// a task even when earlier tasks exist.
    #[test]
    fn toggles_a_late_task_and_respects_fences() {
        let content = "---\ntitle: List\n---\n- [ ] first 📅 2026-12-01 ⏫ #a\n```\n- [ ] in code\n```\n- [ ] last !low #b\n";
        // Line 7 is the last task; its text and markers are parsed.
        let done = set_done(content, 7, "last #b", true).unwrap();
        assert_eq!(
            done,
            "---\ntitle: List\n---\n- [ ] first 📅 2026-12-01 ⏫ #a\n```\n- [ ] in code\n```\n- [x] last !low #b\n"
        );
        // Line 5 looks like a task but sits in a code block.
        assert!(set_done(content, 5, "in code", true).is_none());
        // A stale text on the right line is refused.
        assert!(set_done(content, 7, "something else", true).is_none());
        // The earlier task still toggles, with its own metadata.
        let first = set_done(content, 3, "first #a", true).unwrap();
        assert!(first.contains("- [x] first 📅 2026-12-01 ⏫ #a"));
    }

    #[test]
    fn dates() {
        assert_eq!(normalize_date("2026-12-01"), Some("2026-12-01".into()));
        assert_eq!(
            normalize_date("2026-12-01T10:00:00Z"),
            Some("2026-12-01".into())
        );
        assert_eq!(normalize_date("tomorrow"), None);
    }
}
