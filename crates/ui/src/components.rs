//! One module per component, re-exported flat so callers write
//! `ui::WindowBar`, not `ui::components::window_bar::WindowBar`. Moving a file
//! never breaks an import.

mod delta;
mod sidebar_heading;
mod sidebar_item;
mod window_bar;

pub use delta::*;
pub use sidebar_heading::*;
pub use sidebar_item::*;
pub use window_bar::*;
