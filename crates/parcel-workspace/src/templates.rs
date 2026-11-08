//! Template and export-profile definitions plus the built-in starters.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateNote {
    /// Path within the workspace, e.g. "Notes/Overview.md". Folders are created as needed.
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetadataField {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateSpec {
    pub folders: Vec<String>,
    #[serde(default)]
    pub notes: Vec<TemplateNote>,
    #[serde(default)]
    pub metadata_fields: Vec<MetadataField>,
    #[serde(default)]
    pub default_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub builtin: bool,
    pub spec: TemplateSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProfileSpec {
    pub compress: bool,
    pub verify_after_write: bool,
    pub include_workspace_notes: bool,
    pub include_references: bool,
    /// Case-insensitive file-name globs (`*` and `?`) excluded from packaging, e.g. "*.tmp".
    pub exclude_patterns: Vec<String>,
    /// File name stem pattern; supports {name} and {date}.
    pub filename_pattern: String,
}

impl Default for ProfileSpec {
    fn default() -> Self {
        Self {
            compress: true,
            verify_after_write: true,
            include_workspace_notes: true,
            include_references: true,
            exclude_patterns: vec![],
            filename_pattern: "{name}".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub builtin: bool,
    pub spec: ProfileSpec,
}

fn t(id: &str, name: &str, desc: &str, folders: &[&str], notes: &[(&str, &str)], fields: &[(&str, &str, &str)]) -> Template {
    Template {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        builtin: true,
        spec: TemplateSpec {
            folders: folders.iter().map(|s| s.to_string()).collect(),
            notes: notes.iter().map(|(p, c)| TemplateNote { path: p.to_string(), content: c.to_string() }).collect(),
            metadata_fields: fields
                .iter()
                .map(|(k, l, d)| MetadataField { key: k.to_string(), label: l.to_string(), default: d.to_string() })
                .collect(),
            default_profile: None,
        },
    }
}

pub fn builtin_templates() -> Vec<Template> {
    vec![
        t(
            "tpl-research",
            "Research collection",
            "Sources, readings, data and findings for a research question.",
            &["Sources/Papers", "Sources/Web", "Data", "Analysis", "Figures"],
            &[("Notes/Research question.md", "# Research question\n\nState the question and why it matters.\n\n## Hypotheses\n\n- \n\n## Methods\n\n- \n"),
              ("Notes/Findings.md", "# Findings\n\nRecord what you learn, with references to the entries that support it.\n")],
            &[("topic", "Topic", ""), ("supervisor", "Supervisor / lead", ""), ("status", "Status", "In progress")],
        ),
        t(
            "tpl-design",
            "Design handoff",
            "Everything a developer needs to implement a design.",
            &["Mockups", "Assets/Icons", "Assets/Images", "Fonts", "Specs", "Exports"],
            &[("Notes/Handoff notes.md", "# Handoff notes\n\n## Overview\n\n## Breakpoints\n\n## Interaction details\n\n## Open questions\n")],
            &[("project", "Project", ""), ("designer", "Designer", ""), ("version", "Design version", "1.0")],
        ),
        t(
            "tpl-software",
            "Software project",
            "Source snapshot, docs, configuration and build notes for a project.",
            &["Source", "Docs", "Config", "Tests", "Release"],
            &[("Notes/README.md", "# Project\n\nWhat it does and how to run it.\n\n## Build\n\n## Dependencies\n"),
              ("Notes/Changelog.md", "# Changelog\n\n- \n")],
            &[("repository", "Repository", ""), ("version", "Version", ""), ("license", "License", "")],
        ),
        t(
            "tpl-school",
            "School assignment",
            "Brief, drafts, sources and the final submission in one place.",
            &["Brief", "Drafts", "Sources", "Submission"],
            &[("Notes/Assignment plan.md", "# Assignment plan\n\n## Requirements\n\n- \n\n## Deadline\n\n## Checklist\n\n- [ ] Read the brief\n- [ ] Draft\n- [ ] Cite sources\n- [ ] Submit\n")],
            &[("course", "Course", ""), ("due", "Due date", ""), ("student", "Student", "")],
        ),
        t(
            "tpl-documents",
            "General document collection",
            "A simple structure for gathering related documents.",
            &["Documents", "Images", "Other"],
            &[("Notes/About this collection.md", "# About this collection\n\nWhat these materials are and why they belong together.\n")],
            &[("owner", "Owner", ""), ("purpose", "Purpose", "")],
        ),
    ]
}

pub fn builtin_profiles() -> Vec<ExportProfile> {
    vec![
        ExportProfile {
            id: "prof-standard".into(),
            name: "Standard".into(),
            description: "Compressed, verified after writing.".into(),
            builtin: true,
            spec: ProfileSpec::default(),
        },
        ExportProfile {
            id: "prof-fast".into(),
            name: "Fast (no compression)".into(),
            description: "Stores files without compression; quickest for large media.".into(),
            builtin: true,
            spec: ProfileSpec { compress: false, ..Default::default() },
        },
        ExportProfile {
            id: "prof-clean".into(),
            name: "Clean share".into(),
            description: "Excludes temporary and system files.".into(),
            builtin: true,
            spec: ProfileSpec {
                exclude_patterns: vec!["*.tmp".into(), "~$*".into(), "thumbs.db".into(), ".ds_store".into(), "desktop.ini".into()],
                ..Default::default()
            },
        },
    ]
}

/// Minimal case-insensitive glob supporting `*` and `?`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let s: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut si, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(sp) = star {
            pi = sp + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}
