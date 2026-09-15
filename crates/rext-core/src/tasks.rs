use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl TaskId {
    pub fn fresh() -> Self {
        Self(NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub use_new_terminal: bool,
    #[serde(default)]
    pub allow_concurrent_runs: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariableName {
    File,
    RelativeFile,
    RelativeDir,
    Filename,
    Dirname,
    Stem,
    WorktreeRoot,
    SelectedText,
}

impl VariableName {
    pub fn token(&self) -> &'static str {
        match self {
            VariableName::File => "${file}",
            VariableName::RelativeFile => "${relativeFile}",
            VariableName::RelativeDir => "${relativeDir}",
            VariableName::Filename => "${fileBasename}",
            VariableName::Dirname => "${fileDirname}",
            VariableName::Stem => "${fileBasenameNoExtension}",
            VariableName::WorktreeRoot => "${workspaceFolder}",
            VariableName::SelectedText => "${selectedText}",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TaskContext {
    pub vars: HashMap<VariableName, String>,
}

impl TaskContext {
    pub fn substitute(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (name, value) in &self.vars {
            out = out.replace(name.token(), value);
        }
        out
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedTask {
    pub id: TaskId,
    pub resolved_label: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    pub use_new_terminal: bool,
    pub allow_concurrent_runs: bool,
    pub command_label: String,
}

impl TaskTemplate {
    pub fn resolve(&self, root: &Path, cx: &TaskContext) -> ResolvedTask {
        let command = cx.substitute(&self.command);
        let args: Vec<String> = self.args.iter().map(|a| cx.substitute(a)).collect();
        let cwd = self
            .cwd
            .as_ref()
            .map(|c| root.join(cx.substitute(c)))
            .unwrap_or_else(|| root.to_path_buf());
        let command_label = if args.is_empty() {
            command.clone()
        } else {
            format!("{} {}", command, args.join(" "))
        };
        ResolvedTask {
            id: TaskId::fresh(),
            resolved_label: cx.substitute(&self.label),
            command,
            args,
            env: self.env.clone(),
            cwd,
            use_new_terminal: self.use_new_terminal,
            allow_concurrent_runs: self.allow_concurrent_runs,
            command_label,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct VsCodeTaskFile {
    #[serde(default)]
    tasks: Vec<VsCodeTask>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct VsCodeTask {
    #[serde(default)]
    label: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<VsCodeArg>,
    #[serde(default)]
    options: VsCodeOptions,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum VsCodeArg {
    Str(String),
    Obj { value: String },
}

impl Default for VsCodeArg {
    fn default() -> Self {
        VsCodeArg::Str(String::new())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct VsCodeOptions {
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

pub fn load_vscode_tasks(dir: &Path) -> Vec<TaskTemplate> {
    let path = dir.join(".vscode").join("tasks.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    // Strip // and /* */ comments (JSONC).
    let stripped = strip_json_comments(&text);
    let Ok(file): Result<VsCodeTaskFile, _> = serde_json::from_str(&stripped) else {
        return Vec::new();
    };
    file.tasks
        .into_iter()
        .filter_map(|t| {
            Some(TaskTemplate {
                label: if t.label.is_empty() {
                    t.command.clone()?
                } else {
                    t.label
                },
                command: t.command?,
                args: t
                    .args
                    .into_iter()
                    .map(|a| match a {
                        VsCodeArg::Str(s) => s,
                        VsCodeArg::Obj { value } => value,
                    })
                    .collect(),
                env: t.options.env,
                cwd: t.options.cwd,
                use_new_terminal: false,
                allow_concurrent_runs: false,
            })
        })
        .collect()
}

fn strip_json_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_str = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '/' => match chars.peek() {
                Some('/') => {
                    for nc in chars.by_ref() {
                        if nc == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    chars.next();
                    while let Some(nc) = chars.next() {
                        if nc == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => out.push(c),
            },
            _ => out.push(c),
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct TaskInventory {
    pub templates: Vec<TaskTemplate>,
    pub last_scheduled: Vec<ResolvedTask>,
}

impl TaskInventory {
    pub fn refresh(&mut self, root: &Path) {
        let mut templates = builtin_tasks();
        templates.extend(load_vscode_tasks(root));
        self.templates = templates;
    }

    pub fn schedule(&mut self, template: &TaskTemplate, root: &Path, cx: &TaskContext) -> ResolvedTask {
        let resolved = template.resolve(root, cx);
        self.last_scheduled.retain(|t| {
            t.resolved_label != resolved.resolved_label || resolved.allow_concurrent_runs
        });
        self.last_scheduled.push(resolved.clone());
        if self.last_scheduled.len() > 16 {
            self.last_scheduled.remove(0);
        }
        resolved
    }
}

fn builtin_tasks() -> Vec<TaskTemplate> {
    vec![
        TaskTemplate {
            label: "Cargo: build".into(),
            command: "cargo".into(),
            args: vec!["build".into()],
            ..Default::default()
        },
        TaskTemplate {
            label: "Cargo: test".into(),
            command: "cargo".into(),
            args: vec!["test".into()],
            ..Default::default()
        },
        TaskTemplate {
            label: "Cargo: run current file".into(),
            command: "cargo".into(),
            args: vec!["run".into(), "--bin".into(), "${fileBasenameNoExtension}".into()],
            ..Default::default()
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitute_vars() {
        let mut cx = TaskContext::default();
        cx.vars.insert(VariableName::Filename, "main.rs".into());
        cx.vars.insert(VariableName::Stem, "main".into());
        assert_eq!(
            cx.substitute("run ${fileBasenameNoExtension} < ${fileBasename}"),
            "run main < main.rs"
        );
    }

    #[test]
    fn resolve_template() {
        let t = TaskTemplate {
            label: "build ${workspaceFolder}".into(),
            command: "cargo".into(),
            args: vec!["build".into()],
            ..Default::default()
        };
        let mut cx = TaskContext::default();
        cx.vars
            .insert(VariableName::WorktreeRoot, "/repo".into());
        let r = t.resolve(Path::new("/repo"), &cx);
        assert_eq!(r.resolved_label, "build /repo");
        assert_eq!(r.command_label, "cargo build");
        assert_eq!(r.cwd, Path::new("/repo"));
    }

    #[test]
    fn vscode_tasks_jsonc() {
        let dir = std::env::temp_dir().join("rext_task_test");
        std::fs::create_dir_all(dir.join(".vscode")).unwrap();
        std::fs::write(
            dir.join(".vscode").join("tasks.json"),
            r#"{
            // comment
            "version": "2.0.0",
            "tasks": [
                {"label": "lint", "command": "cargo", "args": ["clippy"]},
                {"label": "docs", "command": "cargo", "args": [{"value": "doc"}]}
            ]
        }"#,
        )
        .unwrap();
        let tasks = load_vscode_tasks(&dir);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].label, "lint");
        assert_eq!(tasks[0].args, &["clippy"]);
        assert_eq!(tasks[1].args, &["doc"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn strip_comments() {
        let out = strip_json_comments(r#"{"a": 1 /* x */} // y"#);
        assert_eq!(out, r#"{"a": 1 } "#);
    }
}
