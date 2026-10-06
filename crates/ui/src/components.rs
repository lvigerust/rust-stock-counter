//! One module per component, re-exported flat so callers write
//! `ui::WindowBar`, not `ui::components::window_bar::WindowBar`. Moving a file
//! never breaks an import.

mod alert_actions;
mod badge;
mod button;
mod delta;
mod dropdown;
mod field;
mod logo;
mod row_button;
mod sidebar;
mod switch_field;
mod text;
mod window_bar;

pub use alert_actions::*;
pub use badge::*;
pub use button::*;
pub use delta::*;
pub use dropdown::*;
pub use field::*;
pub use logo::*;
pub use row_button::*;
pub use sidebar::*;
pub use switch_field::*;
pub use text::*;
pub use window_bar::*;
