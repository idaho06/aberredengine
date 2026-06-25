use std::sync::Arc;

/// Implemented by every GUI widget that carries a named theme key
/// (`GuiWindow`, `GuiButton`, `GuiLabel`). Used by `with_gui_theme_key`
/// in the entity builder to dispatch generically rather than branching
/// per widget type — adding a new themed widget only requires an
/// `impl Themed` here plus one `apply` call in the builder chain.
pub trait Themed {
    fn theme_key_mut(&mut self) -> &mut Arc<str>;
}
