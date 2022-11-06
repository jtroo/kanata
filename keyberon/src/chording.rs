//! Chording implemention to mimic a single key.
//!
//! Provides chord support for emulating a single layout event
//! from multiple key presses. The single event press is triggered
//! once all the keys of the chord have been pressed and the chord
//! is released once all of the keys of the chord have been released.
//!
//! The chording tick should be used after debouncing, where
//! the debounce period determines the period in which all keys
//! need to be pressed to trigger the chord.
//!
//! You must use a virtual row/area of your layout to
//! define the result of the chord if the desired result is
//! not already on the layer that you want to use the chord on.
use crate::layout::Event;
use heapless::Vec;

type KeyPosition = (u8, u16);

/// Description of the virtual key corresponding to a given chord.
/// keys are the coordinates of the multiple keys that make up the chord
/// result is the outcome of the keys being pressed
pub type ChordDef = (KeyPosition, &'static [KeyPosition]);

/// Runtime data for a chord
#[derive(Clone)]
struct Chord {
    def: &'static ChordDef,
    in_progress: bool,
    keys_pressed: Vec<bool, 8>,
}

impl Chord {
    /// Create new chord from user data.
    fn new(def: &'static ChordDef) -> Self {
        let mut me = Self {
            def,
            in_progress: false,
            keys_pressed: Vec::new(),
        };
        for _ in def.1 {
            me.keys_pressed.push(false).unwrap()
        }
        me
    }

    fn tick(&mut self, events: &[Event]) {
        for e in events {
            for (k, _) in self
                .def
                .1
                .iter()
                .enumerate()
                .filter(|(_, key)| **key == e.coord())
            {
                self.keys_pressed[k] = e.is_press();
            }
        }
    }

    fn contains_chord(&mut self, events: &[Event]) -> bool {
        for key in self.def.1 {
            if !events.iter().any(|&k| (&k.coord() == key && k.is_press())) {
                return false;
            }
        }
        true
    }

    fn handle_chord(&mut self, events: &mut Vec<Event, 8>) {
        self.in_progress = true;
        for key in self.def.1 {
            if let Some(position) = events
                .iter()
                .position(|&k| (&k.coord() == key && k.is_press()))
            {
                events.swap_remove(position);
            }
        }
        events
            .push(Event::Press(self.def.0 .0, self.def.0 .1))
            .unwrap();
    }

    fn handle_release(&mut self, events: &mut Vec<Event, 8>) {
        if self.in_progress {
            for key in self.def.1 {
                if let Some(position) = events
                    .iter()
                    .position(|&k| (&k.coord() == key && k.is_release()))
                {
                    events.swap_remove(position);
                }
            }
            if self.keys_pressed.iter().all(|&k| !k) {
                events
                    .push(Event::Release(self.def.0 .0, self.def.0 .1))
                    .unwrap();
                self.in_progress = false;
            }
        }
    }
}

/// The chording manager. Initialize with a list of chord
/// definitions, and update after debounce
pub struct Chording<const N: usize> {
    /// Defined chords
    chords: Vec<Chord, N>,
}

impl<const N: usize> Chording<N> {
    /// Take the predefined chord list in.
    pub fn new(chords: &'static [ChordDef; N]) -> Self {
        Self {
            chords: chords.iter().map(Chord::new).collect(),
        }
    }

    /// Consolidate events and return processed results as a result.
    pub fn tick(&mut self, mut vec: Vec<Event, 8>) -> Vec<Event, 8> {
        for c in &mut self.chords {
            c.tick(&vec);
            if c.contains_chord(&vec) {
                c.handle_chord(&mut vec);
            }
            c.handle_release(&mut vec);
        }
        vec
    }
}

#[cfg(test)]
mod test {
    use super::{ChordDef, Chording};
    use crate::layout::{Event, Event::*};
    use heapless::Vec;

    #[test]
    fn single_press_release() {
        const CHORDS: [ChordDef; 1] = [((0, 2), &[(0, 0), (0, 1)])];
        let mut chording = Chording::new(&CHORDS);

        // Verify a single press goes through chording unchanged
        let mut single_press = Vec::<Event, 8>::new();
        single_press.push(Press(0, 0)).ok();
        assert_eq!(chording.tick(single_press), &[Press(0, 0)]);
        let mut single_release = Vec::<Event, 8>::new();
        single_release.push(Release(0, 0)).ok();
        assert_eq!(chording.tick(single_release), &[Release(0, 0)]);
    }

    #[test]
    fn chord_press_release() {
        const CHORDS: [ChordDef; 1] = [((0, 2), &[(0, 0), (0, 1)])];
        let mut chording = Chording::new(&CHORDS);

        // Verify a chord is converted to the correct key
        let mut double_press = Vec::<Event, 8>::new();
        double_press.push(Press(0, 0)).ok();
        double_press.push(Press(0, 1)).ok();
        assert_eq!(chording.tick(double_press), &[Press(0, 2)]);
        let nothing = Vec::<Event, 8>::new();
        assert_eq!(chording.tick(nothing), &[]);
        let mut double_release = Vec::<Event, 8>::new();
        double_release.push(Release(0, 0)).ok();
        double_release.push(Release(0, 1)).ok();
        let chord_double_release = chording.tick(double_release);
        assert_eq!(chord_double_release, &[Release(0, 2)]);
        let nothing = Vec::<Event, 8>::new();
        assert_eq!(chording.tick(nothing), &[]);
    }

    #[test]
    fn chord_individual_press() {
        const CHORDS: [ChordDef; 1] = [((0, 2), &[(0, 0), (0, 1)])];
        let mut chording = Chording::new(&CHORDS);

        // Verify that pressing the keys that make up a chord at different
        // times will not trigger the chord
        let mut key_a_press = Vec::<Event, 8>::new();
        key_a_press.push(Press(0, 0)).ok();
        assert_eq!(chording.tick(key_a_press), &[Press(0, 0)]);
        let mut key_b_press = Vec::<Event, 8>::new();
        key_b_press.push(Press(0, 1)).ok();
        assert_eq!(chording.tick(key_b_press), &[Press(0, 1)]);
        let nothing = Vec::<Event, 8>::new();
        assert_eq!(chording.tick(nothing), &[]);
    }
    #[test]
    fn chord_press_half_release() {
        const CHORDS: [ChordDef; 1] = [((0, 2), &[(0, 0), (0, 1)])];
        let mut chording = Chording::new(&CHORDS);

        // Verify a chord is converted to the correct key
        let mut double_press = Vec::<Event, 8>::new();
        double_press.push(Press(0, 0)).ok();
        double_press.push(Press(0, 1)).ok();
        assert_eq!(chording.tick(double_press), &[Press(0, 2)]);
        let mut first_release = Vec::<Event, 8>::new();
        first_release.push(Release(0, 0)).ok();
        // we don't want to see the release pass through of a single key
        assert_eq!(chording.tick(first_release), &[]);
        let mut second_release = Vec::<Event, 8>::new();
        second_release.push(Release(0, 1)).ok();
        // once all keys of the combo are released, the combo is released
        assert_eq!(chording.tick(second_release), &[Release(0, 2)]);
    }

    #[test]
    fn chord_overlap_press_release() {
        const CHORDS: [ChordDef; 3] = [
            ((1, 0), &[(0, 0), (0, 1), (0, 2)]),
            ((1, 1), &[(0, 0), (0, 1)]),
            ((1, 2), &[(0, 1), (0, 2)]),
        ];
        let mut chording = Chording::new(&CHORDS);

        // Triple press chord is composed of the two keys that make their
        // own unique chord. Only the three key chord should be triggered
        let mut triple_press = Vec::<Event, 8>::new();
        triple_press.push(Press(0, 0)).ok();
        triple_press.push(Press(0, 1)).ok();
        triple_press.push(Press(0, 2)).ok();
        assert_eq!(chording.tick(triple_press), &[Press(1, 0)]);
        let mut triple_release = Vec::<Event, 8>::new();
        triple_release.push(Release(0, 0)).ok();
        triple_release.push(Release(0, 1)).ok();
        triple_release.push(Release(0, 2)).ok();
        assert_eq!(chording.tick(triple_release), &[Release(1, 0)]);

        // Verifying that the double key chord is pressed and released and not
        // stalled by the overlapping three key chord
        let mut double_press = Vec::<Event, 8>::new();
        double_press.push(Press(0, 0)).ok();
        double_press.push(Press(0, 1)).ok();
        assert_eq!(chording.tick(double_press), &[Press(1, 1)]);
        let mut double_release = Vec::<Event, 8>::new();
        double_release.push(Release(0, 0)).ok();
        double_release.push(Release(0, 1)).ok();
        assert_eq!(chording.tick(double_release), &[Release(1, 1)]);

        // If a three key chord has not been fully released, the released keys
        // that form another chord should still work to press and release the
        // two key chord
        let mut triple_press = Vec::<Event, 8>::new();
        triple_press.push(Press(0, 0)).ok();
        triple_press.push(Press(0, 1)).ok();
        triple_press.push(Press(0, 2)).ok();
        assert_eq!(chording.tick(triple_press), &[Press(1, 0)]);
        let mut half_triple_release = Vec::<Event, 8>::new();
        half_triple_release.push(Release(0, 1)).ok();
        half_triple_release.push(Release(0, 2)).ok();
        assert_eq!(chording.tick(half_triple_release), &[]);
        let mut double_press = Vec::<Event, 8>::new();
        double_press.push(Press(0, 1)).ok();
        double_press.push(Press(0, 2)).ok();
        assert_eq!(chording.tick(double_press), &[Press(1, 2)]);
    }
}
