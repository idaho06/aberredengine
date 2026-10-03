//! [`AssetKind`]: which kind of asset a load command or load reply refers to.

/// The kind of a loaded asset. Each kind has its own key space: a texture and
/// a font may share a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    /// A texture, loaded on the render thread.
    Texture,
    /// A font, loaded on the render thread.
    Font,
    /// A shader, loaded on the render thread.
    Shader,
    /// A sound effect, loaded on the audio thread.
    Sound,
    /// A music stream, loaded on the audio thread.
    Music,
}
