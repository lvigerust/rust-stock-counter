//! Spacing: lengths on Tailwind's scale, where one step is a quarter rem.

use std::ops::{Add, Mul, Sub};

use gpui_kit::{AbsoluteLength, DefiniteLength, Length, Pixels, Rems};

/// A length in steps of Tailwind's spacing scale, a quarter rem each:
/// `Spacing(4.)` is `p-4`'s 1rem and `Spacing(80.)` is `w-80`'s 20rem, 320px
/// at the default rem size. Half steps (`Spacing(1.5)`) are on the scale too.
///
/// It goes wherever GPUI takes a length, and stays in rems until layout, so
/// it scales with the window's rem size as [`Rems`] does:
///
/// ```ignore
/// div().w(Spacing(80.)).p(Spacing(4.)).gap(Spacing(2.))
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spacing(pub f32);

impl Spacing {
    /// How long one step is.
    const STEP: Rems = Rems(0.25);

    /// The same length in rems.
    pub const fn rems(self) -> Rems {
        Rems(self.0 * Self::STEP.0)
    }

    /// The same length in pixels, for a window whose rem is `rem_size`.
    pub fn to_pixels(self, rem_size: Pixels) -> Pixels {
        self.rems().to_pixels(rem_size)
    }
}

impl Add for Spacing {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl Sub for Spacing {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}

impl Mul<f32> for Spacing {
    type Output = Self;

    fn mul(self, factor: f32) -> Self {
        Self(self.0 * factor)
    }
}

impl From<Spacing> for Rems {
    fn from(spacing: Spacing) -> Self {
        spacing.rems()
    }
}

impl From<Spacing> for AbsoluteLength {
    fn from(spacing: Spacing) -> Self {
        spacing.rems().into()
    }
}

impl From<Spacing> for DefiniteLength {
    fn from(spacing: Spacing) -> Self {
        spacing.rems().into()
    }
}

impl From<Spacing> for Length {
    fn from(spacing: Spacing) -> Self {
        spacing.rems().into()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;

    use super::*;

    #[test]
    fn a_step_is_a_quarter_rem() {
        assert_eq!(Spacing(80.).rems(), Rems(20.));
        assert_eq!(Spacing(1.5).rems(), Rems(0.375));
    }

    #[test]
    fn sums_stay_on_the_scale() {
        assert_eq!(Spacing(10.) + Spacing(6.) * 2., Spacing(22.));
        assert_eq!(Spacing(10.) - Spacing(4.), Spacing(6.));
    }

    #[test]
    fn steps_scale_with_the_rem_size() {
        assert_eq!(Spacing(80.).to_pixels(px(16.)), px(320.));
        assert_eq!(Spacing(80.).to_pixels(px(20.)), px(400.));
    }
}
