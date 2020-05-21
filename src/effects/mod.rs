use crate::keycode::KeyCode;
use crate::layers::LayersManager;

type LayerIndex = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
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

impl Effect {
    pub fn press(&self, lmgr: &LayersManager) -> Self {
        match self {
            Default(code) => Effect::Press()
        }
    }
}
