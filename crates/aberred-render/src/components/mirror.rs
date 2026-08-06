//! Components for retained render-world "mirror" entities: one bevy_ecs
//! entity per drawable-list item, keyed by the sim entity's
//! `Entity::to_bits()`, reconciled every time a new `DrawableSnapshot`
//! arrives instead of rebuilt from a `Vec` every frame. The reconciliation
//! logic itself lives in `crate::systems::mirror`.

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

/// Foreign key back to the sim entity a mirror entity was reconciled from.
/// Stores the `Entity` itself (not just its bits) so draw-prep code that
/// needs the original sim `Entity` (e.g. entity-shader uniform seeding) can
/// read it directly with no `Entity::from_bits` reconstruction.
/// `Entity::to_bits()` is only computed where `SimIdMap`'s hashmap key is
/// actually needed.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SimMirror(pub Entity);

/// Category tags. Zero-sized -- distinguish mirror entities that otherwise
/// share `SimMirror` and (for map sprites/texts) the same positional
/// component types, so a `Query<.., With<MirrorX>>` can't accidentally pick
/// up another category's mirrors. `Default` lets `reconcile` spawn any
/// category's marker generically via `Marker::default()`.
#[derive(Component, Debug, Default)]
pub struct MirrorMapSprite;
#[derive(Component, Debug, Default)]
pub struct MirrorMapText;
#[derive(Component, Debug, Default)]
pub struct MirrorScreenSprite;
#[derive(Component, Debug, Default)]
pub struct MirrorScreenText;
#[derive(Component, Debug, Default)]
pub struct MirrorGuiWindow;
#[derive(Component, Debug, Default)]
pub struct MirrorGuiButton;
#[derive(Component, Debug, Default)]
pub struct MirrorGuiLabel;
#[derive(Component, Debug, Default)]
pub struct MirrorGuiProgressBar;

/// `RigidBody.velocity`, captured as a plain component on the mirror entity.
/// Lives here rather than alongside the sim world's other components because
/// it's a reconciliation-layer synthetic type -- nothing in the sim world
/// ever has this component; it wraps the `Vector2` already extracted from
/// `RigidBody` at snapshot-build time (`MapSpriteEntry`/`MapTextEntry`'s
/// `velocity` field). Screen-space categories have no velocity concept.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MirrorVelocity(pub Vector2);
