// GLUE: the scene module the guide's SceneManager `main` declares.
use aberredengine::core::resources::input::InputState;
use aberredengine::core::systems::GameCtx;

pub fn load_assets() {}

pub mod menu {
    use super::*;
    pub fn enter(_ctx: &mut GameCtx) {}
    pub fn update(_ctx: &mut GameCtx, _dt: f32, _input: &InputState) {}
}

pub mod level01 {
    use super::*;
    pub fn enter(_ctx: &mut GameCtx) {}
    pub fn update(_ctx: &mut GameCtx, _dt: f32, _input: &InputState) {}
    pub fn exit(_ctx: &mut GameCtx) {}
}
