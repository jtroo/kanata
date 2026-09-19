//! Analog values and the transformations that turn them into digital edges
//! and continuous motion. Everything here is total and hardware-free.
//!
//! Two conventions are fixed once, so no call site has to remember them: **y
//! is up-positive**, converted at the backend boundary, and **values are
//! clamped, never rejected**, because hardware overshoots its own reported
//! range and refusing such a reading would drop real input.

use core::ops::{Add, AddAssign, Index, IndexMut, Mul};

/// A magnitude in `[0.0, 1.0]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Unit(f32);

impl Unit {
    pub const ZERO: Unit = Unit(0.0);
    pub const ONE: Unit = Unit(1.0);

    /// Clamp a raw reading into the unit interval. Non-finite values become
    /// zero: a controller reporting garbage should read as at rest, not as
    /// fully deflected.
    pub fn new(value: f32) -> Unit {
        match value.is_finite() {
            true => Unit(value.clamp(0.0, 1.0)),
            false => Unit::ZERO,
        }
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

/// A normalized signed axis reading in `[-1.0, 1.0]`.
///
/// Hardware values enter through this constructor, so the projector cannot
/// represent an infinite, NaN, or out-of-range stick position. Non-finite
/// readings mean centered: unlike a large finite overshoot, they carry no
/// recoverable direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct AxisValue(f32);

impl AxisValue {
    pub const ZERO: AxisValue = AxisValue(0.0);

    pub fn new(value: f32) -> AxisValue {
        match value.is_finite() {
            true => AxisValue(value.clamp(-1.0, 1.0)),
            false => AxisValue::ZERO,
        }
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

/// One whole two-axis stick reading, normalized and y-up.
///
/// Keeping the pair together avoids exposing transient half-updated diagonals
/// to projection. [`Vec2`] remains the intentionally unbounded vector used for
/// rates and displacement; this type is only physical stick position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StickPosition {
    x: AxisValue,
    y: AxisValue,
}

impl StickPosition {
    pub fn new(x: f32, y: f32) -> StickPosition {
        StickPosition {
            x: AxisValue::new(x),
            y: AxisValue::new(y),
        }
    }

    pub const fn x(self) -> AxisValue {
        self.x
    }

    pub const fn y(self) -> AxisValue {
        self.y
    }

    pub const fn vector(self) -> Vec2 {
        Vec2 {
            x: self.x.get(),
            y: self.y.get(),
        }
    }
}

/// A two-axis reading, y-up.
///
/// Components are whatever the hardware reported, so the radius of a diagonal
/// may exceed one until a deadzone normalizes it. Also serves as a
/// displacement -- pixels or wheel notches -- and as a pair of axis signs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    /// The identity for [`Motion::invert`]: keep both axes as they are.
    pub const KEEP: Vec2 = Vec2 { x: 1.0, y: 1.0 };

    /// True distance from center, which a hardware corner puts past 1.0.
    pub fn radius(self) -> f32 {
        self.x.hypot(self.y)
    }

    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }

    /// Split into whole units, leaving the fraction in place.
    ///
    /// The pointer path carries the remainder between ticks: a stick asking
    /// for 7.5 pixels a millisecond has to average exactly that, and rounding
    /// a small demand to zero would make slow movement impossible rather than
    /// merely slow.
    pub fn take_whole(&mut self) -> (i32, i32) {
        // This is the only place a bad reading could become lasting state:
        // `NaN - NaN` is NaN, so a single poisoned component would leave the
        // carry NaN and every later take at zero for the rest of the process.
        // `Motion` already refuses to emit one; this is the backstop.
        for axis in [&mut self.x, &mut self.y] {
            if !axis.is_finite() {
                *axis = 0.0;
            }
        }
        let (x, y) = (self.x.trunc(), self.y.trunc());
        self.x -= x;
        self.y -= y;
        (x as i32, y as i32)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, other: Vec2) -> Vec2 {
        Vec2 {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, other: Vec2) {
        *self = *self + other;
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, scale: f32) -> Vec2 {
        Vec2 {
            x: self.x * scale,
            y: self.y * scale,
        }
    }
}

/// Componentwise, which is what applies per-axis inversion.
impl Mul<Vec2> for Vec2 {
    type Output = Vec2;
    fn mul(self, other: Vec2) -> Vec2 {
        Vec2 {
            x: self.x * other.x,
            y: self.y * other.y,
        }
    }
}

/// The response curve applied to continuous projections.
///
/// Digital thresholds are compared *before* any curve, so changing the curve
/// changes pointer feel without silently moving a press point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    /// Output tracks deflection directly.
    Linear,
    /// Squared: finer control near center.
    Quadratic,
    /// Cubed: finest near center, full speed at the rim. The default for
    /// pointer control, where small corrections matter most.
    #[default]
    Cubic,
}

impl Curve {
    pub fn apply(self, t: Unit) -> Unit {
        let t = t.get();
        Unit::new(match self {
            Curve::Linear => t,
            Curve::Quadratic => t * t,
            Curve::Cubic => t * t * t,
        })
    }
}

/// Which continuous output a projection drives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionKind {
    Mouse,
    Scroll,
}

impl MotionKind {
    pub const ALL: [MotionKind; 2] = [MotionKind::Mouse, MotionKind::Scroll];
}

/// How a control's deflection becomes pointer or wheel movement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Radius below which the control reads as at rest. Applied radially
    /// rather than per-axis: an axial deadzone lets a stick held diagonally at
    /// low deflection register on one axis but not the other, which produces
    /// phantom cardinal movement on the way to a diagonal.
    pub deadzone: Unit,
    /// Output at full deflection per second: pixels for the pointer, notches
    /// for the wheel. Converted to the engine's 1 ms tick in [`Motion::rate`].
    pub speed: f32,
    pub curve: Curve,
    /// Per-axis sign. See [`Vec2::KEEP`].
    pub invert: Vec2,
}

impl Motion {
    /// A generous upper bound that still catches an extra zero as a typo.
    pub const MAX_SPEED: f32 = 100_000.0;

    pub const MOUSE: Motion = Motion {
        deadzone: Unit(0.15),
        speed: 1000.0,
        curve: Curve::Cubic,
        // Screen coordinates grow downward while a stick's Y grows upward, so
        // inverting by default makes "push away" mean "move up", which is what
        // every user expects. `(invert-y no)` restores flight-sim behaviour.
        invert: Vec2 { x: 1.0, y: -1.0 },
    };

    pub const SCROLL: Motion = Motion {
        deadzone: Unit(0.15),
        speed: 30.0,
        curve: Curve::Linear,
        // Stick Y and wheel-up are both up-positive, so no inversion.
        invert: Vec2::KEEP,
    };

    pub const fn of(kind: MotionKind) -> Motion {
        match kind {
            MotionKind::Mouse => Motion::MOUSE,
            MotionKind::Scroll => Motion::SCROLL,
        }
    }

    /// The output per 1 ms tick a deflection of `magnitude` asks for.
    ///
    /// The dead region is removed and what survives is rescaled back to
    /// `0..=1`, so the first movement past the edge produces the smallest
    /// possible output instead of jumping straight to the deadzone radius.
    /// Overshoot past the rim clamps, which is also what stops a stick at a
    /// hardware corner from outrunning a straight push.
    pub fn rate(self, magnitude: f32) -> f32 {
        let dz = self.deadzone.get();
        // Non-finite readings are spelled out rather than left to the
        // comparisons below. A disconnecting controller reports NaN, and an
        // infinite radius would clamp to full deflection and then hit the
        // `rate / radius` division in `displace`, where `inf * 0.0` is NaN.
        // They read as at rest rather than as full deflection because no
        // direction can be recovered from them.
        if !magnitude.is_finite() || magnitude <= dz || dz >= 1.0 {
            return 0.0;
        }
        let speed = match self.speed.is_finite() {
            true => self.speed.clamp(0.0, Motion::MAX_SPEED),
            false => 0.0,
        };
        self.curve
            .apply(Unit::new((magnitude - dz) / (1.0 - dz)))
            .get()
            * speed
            / 1000.0
    }

    /// A two-axis reading as a per-tick displacement.
    ///
    /// The curve applies to the *magnitude* rather than to each axis, so the
    /// direction the user is pushing survives; curving the axes independently
    /// would bend a 45-degree push off the diagonal.
    pub fn displace(self, raw: Vec2) -> Vec2 {
        let radius = raw.radius();
        let rate = self.rate(radius);
        match rate == 0.0 {
            true => Vec2::ZERO,
            false => {
                let displaced = raw * (rate / radius) * self.invert;
                Vec2 {
                    x: if displaced.x.is_finite() {
                        displaced.x
                    } else {
                        0.0
                    },
                    y: if displaced.y.is_finite() {
                        displaced.y
                    } else {
                        0.0
                    },
                }
            }
        }
    }
}

/// What the continuous projections are asking for this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Demand([Vec2; 2]);

impl Demand {
    pub fn is_idle(self) -> bool {
        self.0.iter().all(|v| v.is_zero())
    }
}

impl Index<MotionKind> for Demand {
    type Output = Vec2;
    fn index(&self, kind: MotionKind) -> &Vec2 {
        &self.0[kind as usize]
    }
}

impl IndexMut<MotionKind> for Demand {
    fn index_mut(&mut self, kind: MotionKind) -> &mut Vec2 {
        &mut self.0[kind as usize]
    }
}

impl Add for Demand {
    type Output = Demand;
    fn add(self, other: Demand) -> Demand {
        Demand([self.0[0] + other.0[0], self.0[1] + other.0[1]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    fn plain(speed: f32) -> Motion {
        Motion {
            deadzone: Unit::ZERO,
            speed: speed * 1000.0,
            curve: Curve::Linear,
            invert: Vec2::KEEP,
        }
    }

    #[test]
    fn garbage_readings_are_clamped_or_read_as_at_rest() {
        // Hardware overshoots its own range, and a disconnecting controller
        // reports NaN. Treating NaN as full deflection would jam a direction
        // on; letting it reach the deadzone rescale would poison the result.
        assert_eq!(Unit::new(1.4), Unit::ONE);
        assert_eq!(Unit::new(-0.2), Unit::ZERO);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(Unit::new(bad), Unit::ZERO);
        }
        assert_eq!(AxisValue::new(1.4).get(), 1.0);
        assert_eq!(AxisValue::new(-1.4).get(), -1.0);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(AxisValue::new(bad), AxisValue::ZERO);
        }
        assert_eq!(StickPosition::new(2.0, f32::NAN).vector(), v(1.0, 0.0));
        for bad in [v(f32::NAN, 0.0), v(0.0, f32::NAN), v(f32::NAN, f32::NAN)] {
            assert_eq!(Motion::MOUSE.displace(bad), Vec2::ZERO, "{bad:?}");
        }
        for bad_speed in [f32::NAN, f32::INFINITY, -1.0] {
            assert_eq!(
                Motion {
                    speed: bad_speed,
                    ..Motion::MOUSE
                }
                .displace(v(1.0, 0.0)),
                Vec2::ZERO
            );
        }
    }

    #[test]
    fn the_deadzone_silences_the_center_without_a_jump_at_its_edge() {
        let dz = Motion {
            deadzone: Unit::new(0.15),
            ..plain(1.0)
        };
        for inside in [v(0.0, 0.0), v(0.10, 0.0), v(0.10, 0.10), v(-0.14, 0.0)] {
            assert_eq!(dz.displace(inside), Vec2::ZERO, "{inside:?}");
        }
        // Just past the edge is near zero, not near 0.15, and full deflection
        // survives unattenuated.
        assert!(dz.displace(v(0.16, 0.0)).x < 0.02);
        assert!((dz.displace(v(1.0, 0.0)).x - 1.0).abs() < 1e-6);
        // A deadzone of one silences the control instead of dividing by zero.
        let all = Motion {
            deadzone: Unit::ONE,
            ..plain(1.0)
        };
        assert_eq!(all.displace(v(1.0, 1.0)), Vec2::ZERO);
    }

    #[test]
    fn a_diagonal_keeps_its_direction_and_does_not_outrun_a_cardinal() {
        // A hardware corner reads (1, 1), a radius of 1.41. Left alone that
        // makes a diagonal 41% faster than a straight push.
        let m = plain(10.0);
        assert!((m.displace(v(1.0, 0.0)).x - 10.0).abs() < 1e-4);
        for corner in [v(1.0, 1.0), v(-1.0, 1.0), v(1.0, -1.0)] {
            let out = m.displace(corner);
            assert!((out.radius() - 10.0).abs() < 1e-3, "{corner:?} -> {out:?}");
            assert!((out.x.abs() - out.y.abs()).abs() < 1e-4, "off the diagonal");
        }
    }

    #[test]
    fn the_curve_applies_to_the_magnitude_not_to_each_axis() {
        // Curving the axes independently would bend a diagonal toward an axis.
        let cubic = Motion {
            curve: Curve::Cubic,
            ..plain(100.0)
        };
        let s = std::f32::consts::FRAC_1_SQRT_2 * 0.5;
        let out = cubic.displace(v(s, s));
        assert!((out.x - out.y).abs() < 1e-4, "{out:?}");
        // magnitude 0.5, cubed to 0.125, times 100 units per tick.
        assert!((out.radius() - 12.5).abs() < 1e-2, "{out:?}");
        // Steeper curves give finer control near center, and every curve fixes
        // the endpoints.
        let half = Unit::new(0.5);
        assert!(Curve::Cubic.apply(half) < Curve::Quadratic.apply(half));
        assert!(Curve::Quadratic.apply(half) < Curve::Linear.apply(half));
        for curve in [Curve::Linear, Curve::Quadratic, Curve::Cubic] {
            assert_eq!(curve.apply(Unit::ZERO), Unit::ZERO, "{curve:?}");
            assert_eq!(curve.apply(Unit::ONE), Unit::ONE, "{curve:?}");
        }
    }

    #[test]
    fn inversion_flips_only_the_requested_axis() {
        assert!(
            Motion::MOUSE.displace(v(0.0, 1.0)).y < 0.0,
            "push away moves the pointer up the screen"
        );
        assert!(
            Motion::SCROLL.displace(v(0.0, 1.0)).y > 0.0,
            "push away scrolls up"
        );
        assert_eq!(Motion::MOUSE.invert.x, 1.0, "x is not flipped by default");
    }

    #[test]
    fn a_scalar_control_gets_the_same_response_as_an_axis() {
        // A trigger is one number, but the deadzone, curve and speed have to
        // mean the same thing there as on a stick.
        let m = Motion {
            curve: Curve::Quadratic,
            ..plain(8.0)
        };
        assert_eq!(m.rate(0.0), 0.0);
        assert!((m.rate(0.5) - 2.0).abs() < 1e-4, "{}", m.rate(0.5));
        assert!((m.displace(v(1.0, 0.0)).x - m.rate(1.0)).abs() < 1e-4);
    }

    #[test]
    fn a_threshold_is_strict_at_its_boundary() {
        let threshold = Unit::new(0.5);
        assert!(!(Unit::new(0.49) > threshold));
        assert!(!(Unit::new(0.50) > threshold));
        assert!(Unit::new(0.51) > threshold);
    }

    #[test]
    fn demands_sum_per_output_and_report_idleness() {
        let mut a = Demand::default();
        assert!(a.is_idle());
        a[MotionKind::Mouse] = v(1.0, 2.0);
        let mut b = Demand::default();
        b[MotionKind::Mouse] = v(-1.0, 0.5);
        b[MotionKind::Scroll] = v(0.0, 3.0);
        assert!(!a.is_idle());
        let sum = a + b;
        assert_eq!(sum[MotionKind::Mouse], v(0.0, 2.5));
        assert_eq!(sum[MotionKind::Scroll], v(0.0, 3.0));
    }

    #[test]
    fn whole_units_accumulate_exactly_across_ticks() {
        // Rounding each tick independently would make a 7.5 px/ms demand run
        // permanently 7% slow or 25% fast, and a demand below one unit would
        // round to nothing forever.
        let mut carry = Vec2::ZERO;
        let total: i32 = (0..1000)
            .map(|_| {
                carry += v(7.5, 0.0);
                carry.take_whole().0
            })
            .sum();
        assert_eq!(total, 7500);

        // The two axes carry separately: a fast horizontal push must not drag
        // the vertical axis along with it.
        let mut slow = Vec2::ZERO;
        let crawl: i32 = (0..10)
            .map(|_| {
                slow += v(0.25, -0.25);
                slow.take_whole().0
            })
            .sum();
        assert_eq!(crawl, 2, "0.25 a tick over 10 ticks is 2.5 pixels");
        assert_eq!(slow.take_whole(), (0, 0));
    }
}
