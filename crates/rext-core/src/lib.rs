//! UI-free editing core.
//! History captures inverses at apply (Renamite-style), never full rope clones.

mod buffer;
mod display_map;
mod history;
mod selection;
mod workspace;

pub use buffer::{Buffer, BufferId, Point};
pub use display_map::{DisplayMap, DisplayPoint, SoftWrap};
pub use history::{Edit, History};
pub use selection::Selection;
pub use workspace::{BufferHandle, Cmd, Workspace};
