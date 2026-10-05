//! # UI: the app's design layer
//!
//! gpui-kit supplies general-purpose, themed components. This crate sits
//! between it and the features, the way Zed's `ui` crate sits between GPUI and
//! Zed's panels: it holds the few decisions that must look and move the same
//! everywhere in the app, so features compose them instead of restyling
//! gpui-kit at every call site.
//!
//! - styles: focus rings, the dialog frame, motion, spacing and typography,
//!   the parts of the visual language that aren't a single theme color.
//! - components: small presentational pieces (`RenderOnce`), each in its
//!   own module and re-exported flat. The type roles ([`Heading`],
//!   [`Label`], [`Text`], [`SectionHeading`]) and [`Field`] carry Catalyst's
//!   type and spacing; [`Button`], [`Badge`], [`Dropdown`] and the
//!   [`Sidebar`] parts carry its blocks.
//! - [`Assets`]: the app's own images, such as the logo, over gpui-kit's
//!   icons.
//! - [`prelude`]: what a feature file almost always needs.
//!
//! A view composes these and names roles; it doesn't spell out a size, a
//! weight or a focus ring itself. Where no part fits yet, add one here,
//! styled after Catalyst's, and it's the same everywhere from then on. See
//! `docs/design-system.md` for the decisions the parts encode.
//!
//! Nothing here knows about stocktakes. A component that needs domain words
//! (a product's count status, say) belongs in the feature crate.

mod assets;
mod components;
pub mod prelude;
mod styles;

pub use assets::Assets;
pub use components::*;
pub use styles::*;
