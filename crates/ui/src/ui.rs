//! # UI: the app's design layer
//!
//! gpui-kit supplies general-purpose, themed components. This crate sits
//! between it and the features, the way Zed's `ui` crate sits between GPUI and
//! Zed's panels: it holds the few decisions that must look and move the same
//! everywhere in the app, so features compose them instead of restyling
//! gpui-kit at every call site.
//!
//! - [`styles`](crate::styles): focus, motion and typography, the parts of the
//!   visual language that aren't a single theme color.
//! - components: small presentational pieces (`RenderOnce`), each in its
//!   own module and re-exported flat.
//! - [`prelude`]: what a feature file almost always needs.
//!
//! Nothing here knows about stocktakes. A component that needs domain words
//! (a product's count status, say) belongs in the feature crate.

mod components;
pub mod prelude;
mod styles;

pub use components::*;
pub use styles::*;
