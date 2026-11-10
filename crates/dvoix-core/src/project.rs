//! Frontmatter schema for projects. A project is an ordinary note whose
//! frontmatter says `type: project`:
//!
//! ```yaml
//! ---
//! type: project          # or: projekt (German alias)
//! status: active         # idea | active | waiting | done | cancelled
//! due: 2026-12-01        # due date (optional)
//! start: 2026-11-01      # start date (optional)
//! priority: high         # high | medium | low (optional)
//! area: Work             # free-form area (optional)
//! ---
//! ```
//!
//! German values (`aktiv`, `wartend`, `erledigt`, `hoch`, ...) are accepted as
//! well, so existing vaults keep working. The tasks of a project are the
//! checkboxes inside its note.

use serde::Serialize;
use serde_json::Value;

use crate::tasks::{Priority, normalize_date};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Active,
    Waiting,
    Idea,
    Done,
    Cancelled,
}

impl ProjectStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "active" | "aktiv" | "doing" | "in progress" | "in arbeit" | "laufend" => {
                Some(Self::Active)
            }
            "waiting" | "wartend" | "paused" | "pausiert" | "on-hold" | "on hold" | "blocked"
            | "blockiert" => Some(Self::Waiting),
            "idea" | "idee" | "planned" | "geplant" | "todo" | "backlog" => Some(Self::Idea),
            "done" | "erledigt" | "finished" | "fertig" | "abgeschlossen" | "completed" => {
                Some(Self::Done)
            }
            "cancelled" | "abgebrochen" | "canceled" | "dropped" | "verworfen" => {
                Some(Self::Cancelled)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    /// Missing or unknown status counts as active.
    pub status: ProjectStatus,
    pub due: Option<String>,
    pub start: Option<String>,
    pub priority: Option<Priority>,
    pub area: Option<String>,
}

/// Reads the project fields from a note's frontmatter, or `None` if the note is not a project.
pub fn from_frontmatter(fm: &Value) -> Option<ProjectMeta> {
    let ty = fm
        .get("type")
        .and_then(Value::as_str)?
        .trim()
        .to_lowercase();
    if ty != "project" && ty != "projekt" {
        return None;
    }
    let text = |key: &str| match fm.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    };
    Some(ProjectMeta {
        status: text("status")
            .and_then(|s| ProjectStatus::parse(&s))
            .unwrap_or(ProjectStatus::Active),
        due: text("due").and_then(|d| normalize_date(&d)),
        start: text("start").and_then(|d| normalize_date(&d)),
        priority: text("priority").and_then(|p| Priority::parse(&p)),
        area: text("area"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_project_frontmatter() {
        let meta = from_frontmatter(&json!({
            "type": "Project", "status": "waiting", "due": "2026-12-01", "priority": "high", "area": "Work"
        }))
        .unwrap();
        assert_eq!(meta.status, ProjectStatus::Waiting);
        assert_eq!(meta.due.as_deref(), Some("2026-12-01"));
        assert_eq!(meta.priority, Some(Priority::High));
        assert_eq!(meta.area.as_deref(), Some("Work"));
    }

    #[test]
    fn defaults_and_non_projects() {
        let meta = from_frontmatter(&json!({ "type": "project", "status": "???", "due": "bald" }))
            .unwrap();
        assert_eq!(meta.status, ProjectStatus::Active);
        assert_eq!(meta.due, None);
        assert!(from_frontmatter(&json!({ "status": "active" })).is_none());
        assert!(from_frontmatter(&json!({})).is_none());
    }
}
