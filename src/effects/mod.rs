use crate::keycode::KeyCode;

type LayerIndex = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    // Used externally for Press+Release
    Default(KeyCode),

    // Not Implemented Yet
    Sticky(KeyCode),
    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),

    // TODO: Consider how to implement KeyChords.
    // e.g pressing shift-keys ('!', '@', '#').
    // or ctrl-keys ('ctrl-j', 'ctrl-k')
}
