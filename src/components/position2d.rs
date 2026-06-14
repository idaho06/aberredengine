//! Generic 2D position component, parameterized by coordinate space.
//!
//! [`Position2D`] is the shared implementation behind
//! [`MapPosition`](super::mapposition::MapPosition) (world space) and
//! [`ScreenPosition`](super::screenposition::ScreenPosition) (screen space).
//! The two are distinct ECS component types — `Position2D<WorldSpace>` and
//! `Position2D<ScreenSpace>` are different monomorphizations with their own
//! `TypeId` — but share one definition, one set of methods, and one test
//! suite.

use std::marker::PhantomData;

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Marker trait for a 2D coordinate space.
///
/// Implemented only by [`WorldSpace`] and [`ScreenSpace`]. Implementors must
/// be zero-sized unit structs (`PhantomData<S>` is the only field of
/// [`Position2D`] that depends on `S`) — see the note on [`Position2D`]'s
/// manual `Clone`/`Copy` impls before adding a new space.
pub trait PositionSpace: Send + Sync + 'static {}

/// World-space coordinates, used by [`MapPosition`](super::mapposition::MapPosition).
#[derive(Debug, Clone, Copy)]
pub struct WorldSpace;
impl PositionSpace for WorldSpace {}

/// Screen-space (pixel) coordinates, used by [`ScreenPosition`](super::screenposition::ScreenPosition).
#[derive(Debug, Clone, Copy)]
pub struct ScreenSpace;
impl PositionSpace for ScreenSpace {}

/// A 2D position (pivot) for an entity, in the coordinate space `S`.
///
/// `Clone`/`Copy` are implemented manually (rather than derived) so they
/// don't carry a spurious `S: Clone`/`S: Copy` bound — `PhantomData<S>` is
/// `Clone`/`Copy` for any `S`. This matters for `clone_behavior`: it lets
/// `ComponentCloneBehavior::clone::<Self>()` type-check generically over
/// `S: PositionSpace`, so entity cloning (`clone_and_spawn`) copies this
/// component for both [`MapPosition`](super::mapposition::MapPosition) and
/// [`ScreenPosition`](super::screenposition::ScreenPosition).
#[derive(Component, Debug)]
#[component(clone_behavior = clone::<Self>())]
pub struct Position2D<S: PositionSpace> {
    /// 2D coordinates.
    pub pos: Vector2,
    _marker: PhantomData<S>,
}

impl<S: PositionSpace> Clone for Position2D<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: PositionSpace> Copy for Position2D<S> {}

impl<S: PositionSpace> Default for Position2D<S> {
    fn default() -> Self {
        Self {
            pos: Vector2::default(),
            _marker: PhantomData,
        }
    }
}

impl<S: PositionSpace> Position2D<S> {
    /// Create a position from x and y.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            pos: Vector2 { x, y },
            _marker: PhantomData,
        }
    }

    /// Create a position from an existing Vector2.
    pub fn from_vec(pos: Vector2) -> Self {
        Self {
            pos,
            _marker: PhantomData,
        }
    }

    /// Get the underlying Vector2.
    pub fn pos(&self) -> Vector2 {
        self.pos
    }

    /// X coordinate.
    pub fn x(&self) -> f32 {
        self.pos.x
    }

    /// Y coordinate.
    pub fn y(&self) -> f32 {
        self.pos.y
    }

    /// Set the entire position.
    pub fn set_pos(&mut self, pos: Vector2) {
        self.pos = pos;
    }

    /// Set X coordinate.
    pub fn set_x(&mut self, x: f32) {
        self.pos.x = x;
    }

    /// Set Y coordinate.
    pub fn set_y(&mut self, y: f32) {
        self.pos.y = y;
    }

    /// Translate by delta.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.pos.x += dx;
        self.pos.y += dy;
    }

    /// Builder-style: return a copy with a different X.
    pub fn with_x(mut self, x: f32) -> Self {
        self.pos.x = x;
        self
    }

    /// Builder-style: return a copy with a different Y.
    pub fn with_y(mut self, y: f32) -> Self {
        self.pos.y = y;
        self
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    /// Exercises the full `Position2D` API for the given coordinate space.
    /// Called once per space (`WorldSpace`, `ScreenSpace`) from
    /// `mapposition`/`screenposition`'s own test modules.
    pub(crate) fn run_shared_tests<S: PositionSpace>() {
        // new
        let pos = Position2D::<S>::new(10.0, 20.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));

        // new with zero
        let pos = Position2D::<S>::new(0.0, 0.0);
        assert!(approx_eq(pos.pos.x, 0.0));
        assert!(approx_eq(pos.pos.y, 0.0));

        // new with negative values
        let pos = Position2D::<S>::new(-5.0, -10.0);
        assert!(approx_eq(pos.pos.x, -5.0));
        assert!(approx_eq(pos.pos.y, -10.0));

        // default is zero
        let pos = Position2D::<S>::default();
        assert!(approx_eq(pos.pos.x, 0.0));
        assert!(approx_eq(pos.pos.y, 0.0));

        // from_vec
        let vec = Vector2 { x: 15.0, y: 25.0 };
        let pos = Position2D::<S>::from_vec(vec);
        assert!(approx_eq(pos.pos.x, 15.0));
        assert!(approx_eq(pos.pos.y, 25.0));

        // pos getter
        let pos = Position2D::<S>::new(1.0, 2.0);
        let vec = pos.pos();
        assert!(approx_eq(vec.x, 1.0));
        assert!(approx_eq(vec.y, 2.0));

        // x / y getters
        let pos = Position2D::<S>::new(7.0, 8.0);
        assert!(approx_eq(pos.x(), 7.0));
        assert!(approx_eq(pos.y(), 8.0));

        // set_pos
        let mut pos = Position2D::<S>::new(0.0, 0.0);
        pos.set_pos(Vector2 { x: 100.0, y: 200.0 });
        assert!(approx_eq(pos.pos.x, 100.0));
        assert!(approx_eq(pos.pos.y, 200.0));

        // set_x
        let mut pos = Position2D::<S>::new(1.0, 2.0);
        pos.set_x(99.0);
        assert!(approx_eq(pos.pos.x, 99.0));
        assert!(approx_eq(pos.pos.y, 2.0)); // y unchanged

        // set_y
        let mut pos = Position2D::<S>::new(1.0, 2.0);
        pos.set_y(99.0);
        assert!(approx_eq(pos.pos.x, 1.0)); // x unchanged
        assert!(approx_eq(pos.pos.y, 99.0));

        // translate
        let mut pos = Position2D::<S>::new(10.0, 20.0);
        pos.translate(5.0, -3.0);
        assert!(approx_eq(pos.pos.x, 15.0));
        assert!(approx_eq(pos.pos.y, 17.0));

        // translate with zero
        let mut pos = Position2D::<S>::new(10.0, 20.0);
        pos.translate(0.0, 0.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));

        // with_x builder
        let pos = Position2D::<S>::new(1.0, 2.0).with_x(50.0);
        assert!(approx_eq(pos.pos.x, 50.0));
        assert!(approx_eq(pos.pos.y, 2.0));

        // with_y builder
        let pos = Position2D::<S>::new(1.0, 2.0).with_y(50.0);
        assert!(approx_eq(pos.pos.x, 1.0));
        assert!(approx_eq(pos.pos.y, 50.0));

        // builder chaining
        let pos = Position2D::<S>::new(0.0, 0.0).with_x(10.0).with_y(20.0);
        assert!(approx_eq(pos.pos.x, 10.0));
        assert!(approx_eq(pos.pos.y, 20.0));
    }
}
