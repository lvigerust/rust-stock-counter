//! One module per component, re-exported flat so callers write
//! `ui::WindowBar`, not `ui::components::window_bar::WindowBar`. Moving a file
//! never breaks an import.

mod button;
mod delta;
mod dropdown;
mod logo;
mod row_button;
mod sidebar;
mod window_bar;

pub use button::*;
pub use delta::*;
pub use dropdown::*;
pub use logo::*;
pub use row_button::*;
pub use sidebar::*;
pub use window_bar::*;
