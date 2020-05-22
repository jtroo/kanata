use crate::keys::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    // Used externally for Press+Release
    Default(KeyCode),

    // Not Implemented Yet
    // ---------------------
    // ToggleLayer(LayerIndex),
    // OneShotLayer(LayerIndex),

    // ToggleModifier(KeyCode)
    // OneShotModifier(KeyCode)

    // TODO: Consider how to implement KeyChords.
    // e.g pressing shift-keys ('!', '@', '#').
    // or ctrl-keys ('ctrl-j', 'ctrl-k')
}
