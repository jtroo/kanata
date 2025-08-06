//! Contains code to handle global override keys.

use anyhow::{Result, anyhow, bail};
use rustc_hash::FxHashMap as HashMap;

use crate::keys::*;

use kanata_keyberon::key_code::KeyCode;
use kanata_keyberon::layout::NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION;
use kanata_keyberon::layout::NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE;
use kanata_keyberon::layout::State;

/// Scratch space containing allocations used to process override information. Exists as an
/// optimization to reuse allocations between iterations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverrideStates {
    mods_pressed: u8,
    oscs_to_remove: Vec<OsCode>,
    oscs_to_add: Vec<OsCode>,
}

impl Default for OverrideStates {
    fn default() -> Self {
        Self::new()
    }
}

impl OverrideStates {
    pub fn new() -> Self {
        Self {
            mods_pressed: 0,
            oscs_to_add: Vec::new(),
            oscs_to_remove: Vec::new(),
        }
    }

    fn cleanup(&mut self) {
        self.oscs_to_add.clear();
        self.oscs_to_remove.clear();
        self.mods_pressed = 0;
    }

    fn update(&mut self, osc: OsCode, overrides: &Overrides) {
        if let Some(mod_mask) = mask_for_key(osc) {
            self.mods_pressed |= mod_mask;
        } else {
            overrides.update_keys(
                osc,
                self.mods_pressed,
                &mut self.oscs_to_add,
                &mut self.oscs_to_remove,
            );
        }
    }

    fn is_key_overridden(&self, osc: OsCode) -> bool {
        self.oscs_to_remove.contains(&osc)
    }

    fn add_overrides(&self, oscs: &mut Vec<KeyCode>) {
        oscs.extend(self.oscs_to_add.iter().copied().map(KeyCode::from));
    }

    pub fn removed_oscs(&self) -> impl Iterator<Item = OsCode> + '_ {
        self.oscs_to_remove.iter().copied()
    }
}

/// A collection of global key overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overrides {
    overrides_by_osc: HashMap<OsCode, Vec<Override>>,
}

impl Overrides {
    pub fn new(overrides: &[Override]) -> Self {
        let mut overrides_by_osc: HashMap<OsCode, Vec<Override>> = HashMap::default();
        for o in overrides.iter() {
            overrides_by_osc
                .entry(o.in_non_mod_osc)
                .and_modify(|ovd| ovd.push(o.clone()))
                .or_insert_with(|| vec![o.clone()]);
        }
        for ovds in overrides_by_osc.values_mut() {
            ovds.shrink_to_fit();
        }
        overrides_by_osc.shrink_to_fit();
        Self { overrides_by_osc }
    }

    pub fn override_keys(&self, kcs: &mut Vec<KeyCode>, states: &mut OverrideStates) {
        if self.is_empty() {
            return;
        }
        states.cleanup();
        for kc in kcs.iter().copied() {
            states.update(kc.into(), self);
        }
        kcs.retain(|kc| !states.is_key_overridden((*kc).into()));
        states.add_overrides(kcs);
    }

    pub fn output_non_mods_for_input_non_mod(&self, in_osc: OsCode) -> Vec<OsCode> {
        let mut ret = Vec::new();
        if let Some(ovds) = self.overrides_by_osc.get(&in_osc) {
            for out_osc in ovds.iter().map(|ovd| ovd.out_non_mod_osc) {
                ret.push(out_osc);
            }
        }
        ret
    }

    fn is_empty(&self) -> bool {
        self.overrides_by_osc.is_empty()
    }

    fn update_keys(
        &self,
        active_osc: OsCode,
        active_mod_mask: u8,
        oscs_to_add: &mut Vec<OsCode>,
        oscs_to_remove: &mut Vec<OsCode>,
    ) {
        let Some(ovds) = self.overrides_by_osc.get(&active_osc) else {
            return;
        };
        let mut cur_chord_size = 0;
        if let Some(ovd) = ovds
            .iter()
            .filter(|ovd| {
                let mask = ovd.get_mod_mask();
                if mask & active_mod_mask == mask {
                    // keep only the longest matching prefix.
                    let chord_size = ovd.in_mod_oscs.len() + 1;
                    if chord_size <= cur_chord_size {
                        false
                    } else {
                        cur_chord_size = chord_size;
                        true
                    }
                } else {
                    false
                }
            })
            .next_back()
        {
            log::debug!("using override {ovd:?}");
            ovd.add_override_keys(oscs_to_add);
            ovd.add_removed_keys(oscs_to_remove);
        }
    }
}

/// A global key override.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Override {
    in_non_mod_osc: OsCode,
    out_non_mod_osc: OsCode,
    in_mod_oscs: Vec<OsCode>,
    out_mod_oscs: Vec<OsCode>,
}

impl Override {
    pub fn try_new(in_oscs: &[OsCode], out_oscs: &[OsCode]) -> Result<Self> {
        let mut in_nmoscs = in_oscs
            .iter()
            .copied()
            .filter(|osc| mask_for_key(*osc).is_none());
        let in_non_mod_osc = in_nmoscs.next().ok_or_else(|| {
            anyhow!("override must contain exactly one input non-modifier key; found none")
        })?;
        if in_nmoscs.next().is_some() {
            bail!("override must contain exactly one input non-modifier key; found multiple");
        }
        let mut out_nmoscs = out_oscs
            .iter()
            .copied()
            .filter(|osc| mask_for_key(*osc).is_none());
        let out_non_mod_osc = out_nmoscs.next().ok_or_else(|| {
            anyhow!("override must contain exactly one output non-modifier key; found none")
        })?;
        if out_nmoscs.next().is_some() {
            bail!("override must contain exactly one output non-modifier key; found multiple");
        }
        let mut in_mod_oscs = in_oscs
            .iter()
            .copied()
            .filter(|osc| mask_for_key(*osc).is_some())
            .collect::<Vec<_>>();
        let mut out_mod_oscs = out_oscs
            .iter()
            .copied()
            .filter(|osc| mask_for_key(*osc).is_some())
            .collect::<Vec<_>>();
        in_mod_oscs.shrink_to_fit();
        out_mod_oscs.shrink_to_fit();
        Ok(Self {
            in_non_mod_osc,
            out_non_mod_osc,
            in_mod_oscs,
            out_mod_oscs,
        })
    }

    fn get_mod_mask(&self) -> u8 {
        let mut mask = 0;
        for osc in self.in_mod_oscs.iter().copied() {
            mask |= mask_for_key(osc).expect("mod only");
        }
        mask
    }

    fn add_override_keys(&self, oscs_to_add: &mut Vec<OsCode>) {
        for osc in self.out_mod_oscs.iter().copied() {
            if !oscs_to_add.contains(&osc) {
                oscs_to_add.push(osc);
            }
        }
        if !oscs_to_add.contains(&self.out_non_mod_osc) {
            oscs_to_add.push(self.out_non_mod_osc);
        }
    }

    fn add_removed_keys(&self, oscs_to_remove: &mut Vec<OsCode>) {
        for osc in self.in_mod_oscs.iter().copied() {
            if !oscs_to_remove.contains(&osc) {
                oscs_to_remove.push(osc);
            }
        }
        if !oscs_to_remove.contains(&self.in_non_mod_osc) {
            oscs_to_remove.push(self.in_non_mod_osc);
        }
    }
}

fn mask_for_key(osc: OsCode) -> Option<u8> {
    match osc {
        OsCode::KEY_LEFTCTRL => Some(1 << 0),
        OsCode::KEY_LEFTSHIFT => Some(1 << 1),
        OsCode::KEY_LEFTALT => Some(1 << 2),
        OsCode::KEY_LEFTMETA => Some(1 << 3),
        OsCode::KEY_RIGHTCTRL => Some(1 << 4),
        OsCode::KEY_RIGHTSHIFT => Some(1 << 5),
        OsCode::KEY_RIGHTALT => Some(1 << 6),
        OsCode::KEY_RIGHTMETA => Some(1 << 7),
        _ => None,
    }
}

/// For every `OsCode` marked for removal by overrides that is not a modifier,
/// mark its state in the keyberon layout
/// with `NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION` and `NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE`
/// so that it gets eagerly cleared, avoiding weird character outputs.
pub fn mark_overridden_nonmodkeys_for_eager_erasure<T>(
    override_states: &OverrideStates,
    kb_states: &mut [State<T>],
) {
    for osc_to_mark in override_states
        .removed_oscs()
        .filter(|osc| !osc.is_modifier())
    {
        let kc: KeyCode = osc_to_mark.into();
        for kbstate in kb_states.iter_mut() {
            if let State::NormalKey {
                mut flags,
                keycode,
                coord,
            } = kbstate
            {
                if kc == *keycode {
                    flags.0 |= NORMAL_KEY_FLAG_CLEAR_ON_NEXT_ACTION
                        | NORMAL_KEY_FLAG_CLEAR_ON_NEXT_RELEASE;
                    *kbstate = State::NormalKey {
                        flags,
                        keycode: *keycode,
                        coord: *coord,
                    };
                }
            }
        }
    }
}
