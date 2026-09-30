//! One module per component, re-exported flat so callers write
//! `ui::KeyHint`, not `ui::components::key_hint::KeyHint`. Moving a file
//! never breaks an import.

mod card;
mod delta;
mod key_hint;
mod stat;

pub use card::*;
pub use delta::*;
pub use key_hint::*;
pub use stat::*;
