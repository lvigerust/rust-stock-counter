//! One module per component, re-exported flat so callers write
//! `ui::KeyHint`, not `ui::components::key_hint::KeyHint`. Moving a file
//! never breaks an import.

mod delta;
mod key_hint;
mod progress_meter;

pub use delta::*;
pub use key_hint::*;
pub use progress_meter::*;
