//! UI-free editing core.
//! History captures inverses at apply (Renamite-style), never full rope clones.

mod buffer;
mod diagnostics;
mod display_map;
mod history;
mod selection;
mod snippet;
mod tasks;
mod workspace;

pub use buffer::{Buffer, BufferId, Point};
pub use diagnostics::{Diagnostic, DiagnosticEntry, DiagnosticSet, DiagnosticsStore, Severity};
pub use display_map::{DisplayMap, DisplayPoint, SoftWrap};
pub use history::{Edit, History};
pub use selection::{Selection, SelectionGoal, Selections};
pub use snippet::{Snippet, SnippetError, TabStop};
pub use tasks::{ResolvedTask, TaskContext, TaskId, TaskInventory, TaskTemplate, VariableName, load_vscode_tasks};
pub use workspace::{BufferHandle, Cmd, Workspace, clamp_point_pub};
