// GLUE: the scene module the guide's SceneManager `main` declares.
use aberredengine::prelude::*;

pub fn load_assets() {}

pub mod menu {
    use super::*;
    pub fn enter(_: On<SceneEntered>) {}
    pub fn update() {}
}

pub mod level01 {
    use super::*;
    pub fn enter(_: On<SceneEntered>) {}
    pub fn update() {}
    pub fn exit(_: On<SceneExited>) {}
}
