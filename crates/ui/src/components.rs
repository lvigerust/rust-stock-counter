//! One module per component, re-exported flat so callers write
//! `ui::KeyHint`, not `ui::components::key_hint::KeyHint`. Moving a file
//! never breaks an import.

mod delta;
mod key_hint;
mod sidebar_heading;
mod sidebar_item;
mod stat;
mod window_bar;

pub use delta::*;
pub use key_hint::*;
pub use sidebar_heading::*;
pub use sidebar_item::*;
pub use stat::*;
pub use window_bar::*;
