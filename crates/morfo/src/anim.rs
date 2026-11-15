//! Analytic spring animation.
//!
//! A damped spring has a closed-form solution that is *linear* in the initial
//! offset and velocity: `x(t) = a·x0 + b·v0`, `v(t) = c·x0 + d·v0`. The four
//! coefficients depend only on the spring and the time step, so they are
//! computed **once per frame** ([`SpringStep`]) and then every animated value
//! in the interface advances with four multiplications per component.
//!
//! Consequences: exact at any frame rate (no integration error, no explosion
//! on long frames), interruptible at any moment with velocity preserved, and
//! values at rest cost nothing.

use crate::geom::{Color, Rect, Vec2};

/// Spring described by its perceptual `response` time and damping ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Undamped angular frequency (rad/s).
    pub omega: f32,
    /// Damping ratio: `1` critical (no overshoot), `<1` bouncy, `>1` sluggish.
    pub zeta: f32,
}

impl Spring {
    /// `response`: approximate duration in seconds. `damping`: damping ratio.
    pub const fn new(response: f32, damping: f32) -> Self {
        Spring { omega: std::f32::consts::TAU / response, zeta: damping }
    }
    /// Hover, press: immediate feedback.
    pub const SNAPPY: Spring = Spring::new(0.20, 1.0);
    /// Layout, color and shape changes.
    pub const SMOOTH: Spring = Spring::new(0.42, 0.92);
    /// Large surfaces, reveals, theme changes.
    pub const GENTLE: Spring = Spring::new(0.62, 1.0);
    /// Playful overshoot: toggles, selection indicators.
    pub const BOUNCY: Spring = Spring::new(0.45, 0.62);
}

/// The built-in springs every [`Ui`](crate::Ui) precomputes each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpringKind {
    Snappy = 0,
    Smooth = 1,
    Gentle = 2,
    Bouncy = 3,
}

/// Precomputed advance of a [`Spring`] over a time step.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpringStep {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
}

impl SpringStep {
    pub fn new(s: Spring, dt: f32) -> Self {
        let (w, z, t) = (s.omega, s.zeta, dt.max(0.0));
        if z < 0.999 {
            let wd = w * (1.0 - z * z).sqrt();
            let al = z * w;
            let e = (-al * t).exp();
            let (sin, cos) = (wd * t).sin_cos();
            SpringStep {
                a: e * (cos + al / wd * sin),
                b: e * sin / wd,
                c: -e * w * w / wd * sin,
                d: e * (cos - al / wd * sin),
            }
        } else if z <= 1.001 {
            let e = (-w * t).exp();
            SpringStep { a: e * (1.0 + w * t), b: e * t, c: -e * w * w * t, d: e * (1.0 - w * t) }
        } else {
            let s = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w * (z - s), -w * (z + s));
            let (e1, e2) = ((r1 * t).exp(), (r2 * t).exp());
            let inv = 1.0 / (r2 - r1);
            SpringStep {
                a: (r2 * e1 - r1 * e2) * inv,
                b: (e2 - e1) * inv,
                c: r1 * r2 * (e1 - e2) * inv,
                d: (r2 * e2 - r1 * e1) * inv,
            }
        }
    }

    #[inline(always)]
    fn apply(&self, value: &mut f32, velocity: &mut f32, target: f32) -> f32 {
        let x = *value - target;
        let v = *velocity;
        let nx = self.a * x + self.b * v;
        let nv = self.c * x + self.d * v;
        *value = target + nx;
        *velocity = nv;
        nx.abs().max(nv.abs() * 0.02)
    }
}

/// A value that can be driven by a spring.
pub trait Animatable: Copy + PartialEq {
    const ZERO: Self;
    /// Advances `value`/`velocity` toward `target`; returns the remaining motion.
    fn advance(value: &mut Self, velocity: &mut Self, target: &Self, k: &SpringStep) -> f32;
}

impl Animatable for f32 {
    const ZERO: Self = 0.0;
    #[inline]
    fn advance(value: &mut Self, velocity: &mut Self, target: &Self, k: &SpringStep) -> f32 {
        k.apply(value, velocity, *target)
    }
}

macro_rules! animatable_struct {
    ($ty:ty, $zero:expr, $($f:ident),+) => {
        impl Animatable for $ty {
            const ZERO: Self = $zero;
            #[inline]
            fn advance(value: &mut Self, velocity: &mut Self, target: &Self, k: &SpringStep) -> f32 {
                let mut rest = 0.0f32;
                $( rest = rest.max(k.apply(&mut value.$f, &mut velocity.$f, target.$f)); )+
                rest
            }
        }
    };
}

animatable_struct!(Vec2, Vec2::ZERO, x, y);
animatable_struct!(Rect, Rect::new(0.0, 0.0, 0.0, 0.0), x, y, w, h);
animatable_struct!(Color, Color::TRANSPARENT, r, g, b, a);

impl<T: Animatable, const N: usize> Animatable for [T; N] {
    const ZERO: Self = [T::ZERO; N];
    #[inline]
    fn advance(value: &mut Self, velocity: &mut Self, target: &Self, k: &SpringStep) -> f32 {
        let mut rest = 0.0f32;
        for i in 0..N {
            rest = rest.max(T::advance(&mut value[i], &mut velocity[i], &target[i], k));
        }
        rest
    }
}

/// Below this much remaining motion a value snaps to its target and sleeps.
const REST: f32 = 0.002;

/// A spring-driven value: retarget it whenever you like, it never jumps.
#[derive(Clone, Copy, Debug)]
pub struct Motion<T> {
    value: T,
    velocity: T,
    target: T,
    resting: bool,
}

impl<T: Animatable> Motion<T> {
    pub fn new(value: T) -> Self {
        Motion { value, velocity: T::ZERO, target: value, resting: true }
    }
    #[inline]
    pub fn get(&self) -> T {
        self.value
    }
    pub fn target(&self) -> T {
        self.target
    }
    pub fn is_resting(&self) -> bool {
        self.resting
    }
    /// Sets a new destination, keeping the current position and velocity.
    #[inline]
    pub fn set(&mut self, target: T) {
        if target != self.target {
            self.target = target;
            self.resting = false;
        }
    }
    /// Jumps to `value` with no animation.
    pub fn snap(&mut self, value: T) {
        self.value = value;
        self.target = value;
        self.velocity = T::ZERO;
        self.resting = true;
    }
    /// Advances the value. Returns `true` while it is still moving.
    #[inline]
    pub fn tick(&mut self, k: &SpringStep) -> bool {
        if self.resting {
            return false;
        }
        if T::advance(&mut self.value, &mut self.velocity, &self.target, k) < REST {
            self.value = self.target;
            self.velocity = T::ZERO;
            self.resting = true;
        }
        // A change too small to see settles at once and is not an animation:
        // slowly drifting inputs (a clock, a download) must not keep the
        // interface awake.
        !self.resting
    }
    /// `set` + `tick`, or `snap` when `fresh` (first frame of an element).
    #[inline]
    pub fn drive(&mut self, target: T, fresh: bool, k: &SpringStep) -> bool {
        if fresh {
            self.snap(target);
            false
        } else {
            self.set(target);
            self.tick(k)
        }
    }
}

/// Smooth 0..1 ramp for cross-fades driven by a spring progress.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_and_rests() {
        for spring in [Spring::SNAPPY, Spring::SMOOTH, Spring::GENTLE, Spring::BOUNCY, Spring::new(0.5, 1.8)] {
            let k = SpringStep::new(spring, 1.0 / 60.0);
            let mut m = Motion::new(0.0f32);
            m.set(100.0);
            let mut frames = 0;
            while m.tick(&k) {
                frames += 1;
                assert!(frames < 600, "spring never settled");
            }
            assert_eq!(m.get(), 100.0);
            assert!(m.is_resting());
        }
    }

    #[test]
    fn frame_rate_independent() {
        // One 100 ms step must land where ten 10 ms steps do.
        for spring in [Spring::SMOOTH, Spring::BOUNCY, Spring::new(0.5, 1.8)] {
            let (mut x1, mut v1) = (0.0f32, 3.0f32);
            SpringStep::new(spring, 0.1).apply(&mut x1, &mut v1, 50.0);
            let (mut x2, mut v2) = (0.0f32, 3.0f32);
            let k = SpringStep::new(spring, 0.01);
            for _ in 0..10 {
                k.apply(&mut x2, &mut v2, 50.0);
            }
            assert!((x1 - x2).abs() < 1e-2, "{x1} vs {x2}");
            assert!((v1 - v2).abs() < 0.5, "{v1} vs {v2}");
        }
    }

    #[test]
    fn imperceptible_changes_do_not_animate() {
        let k = SpringStep::new(Spring::SMOOTH, 1.0 / 60.0);
        let mut m = Motion::new(0.5f32);
        for i in 1..100 {
            m.set(0.5 + i as f32 * 1e-5);
            assert!(!m.tick(&k));
        }
        assert_eq!(m.get(), m.target());
    }

    #[test]
    fn retarget_keeps_velocity() {
        let k = SpringStep::new(Spring::SMOOTH, 1.0 / 60.0);
        let mut m = Motion::new(0.0f32);
        m.set(100.0);
        for _ in 0..5 {
            m.tick(&k);
        }
        let before = m.get();
        m.set(-100.0);
        m.tick(&k);
        // Still carried forward by its momentum for a moment: no jump back.
        assert!(m.get() > before - 5.0);
    }
}
