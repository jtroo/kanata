use super::*;

impl Kanata {
    /// This compares the active keys in the keyberon layout against the potential key outputs for
    /// corresponding physical key in the configuration. If any of keyberon active keys match any
    /// potential physical key output, write the repeat event to the OS.
    pub(super) fn handle_repeat(&mut self, event: &KeyEvent) -> Result<()> {
        let ret = self.handle_repeat_actual(event);
        // The cur_keys Vec is re-used for processing, for efficiency reasons to avoid allocation.
        // Unlike prev_keys which has useful info for the next call to handle_time_ticks, cur_keys
        // can be reused and cleared — it just needs to be empty for the next handle_time_ticks
        // call.
        self.cur_keys.clear();
        ret
    }

    pub(super) fn handle_repeat_actual(
        &mut self,
        event: &KeyEvent,
    ) -> Result<()> {
        if let Some(state) = self.sequence_state.get_active() {
            // While in non-visible sequence mode, don't send key repeats. I can't imagine it's a
            // helpful use case for someone trying to type in a sequence that they want to rely on
            // key repeats to finish a sequence. I suppose one might want to do repeat in order to
            // try and cancel an input sequence... I'll wait for a user created issue to deal with
            // this.
            //
            // It should be noted that even with visible-backspaced, key repeat does not interact
            // with the sequence; the key is output with repeat as normal. Which might be
            // surprising/unexpected. It's technically fixable but I don't want to add the code to
            // do that if nobody needs it.
            if state.sequence_input_mode != SequenceInputMode::VisibleBackspaced
            {
                return Ok(());
            }
        }
        self.cur_keys.extend(self.layout.bm().keycodes());
        self.overrides
            .override_keys(&mut self.cur_keys, &mut self.override_states);

        // Prioritize checking the active layer in case a layer-while-held is active.
        let active_held_layers =
            self.layout.bm().trans_resolution_layer_order();
        let mut held_layer_active = false;
        for layer in active_held_layers {
            held_layer_active = true;
            if let Some(outputs_for_key) =
                self.key_outputs[usize::from(layer)].get(&event.code)
            {
                log::debug!("key outs for active layer-while-held: {outputs_for_key:?};");
                for osc in outputs_for_key.iter().rev().copied() {
                    let kc = osc.into();
                    if self.cur_keys.contains(&kc)
                        || self.unshifted_keys.contains(&kc)
                        || self.unmodded_keys.contains(&kc)
                    {
                        log::debug!("repeat    {:?}", KeyCode::from(osc));
                        if let Err(e) =
                            write_key(&mut self.kbd_out, osc, KeyValue::Repeat)
                        {
                            bail!("could not write key {e:?}")
                        }
                        return Ok(());
                    }
                }
            }
        }
        if held_layer_active {
            log::debug!("empty layer-while-held outputs, probably transparent");
        }

        if let Some(outputs_for_key) =
            self.key_outputs[self.layout.bm().default_layer].get(&event.code)
        {
            // Try matching a key on the default layer.
            //
            // This code executes in two cases:
            // 1. current layer is the default layer
            // 2. current layer is layer-while-held but did not find a match in the code above, e.g. a
            //    transparent key was pressed.
            log::debug!("key outs for default layer: {outputs_for_key:?};");
            for osc in outputs_for_key.iter().rev().copied() {
                let kc = osc.into();
                if self.cur_keys.contains(&kc)
                    || self.unshifted_keys.contains(&kc)
                    || self.unmodded_keys.contains(&kc)
                {
                    log::debug!("repeat    {:?}", KeyCode::from(osc));
                    if let Err(e) =
                        write_key(&mut self.kbd_out, osc, KeyValue::Repeat)
                    {
                        bail!("could not write key {e:?}")
                    }
                    return Ok(());
                }
            }
        }

        // Reached here and have not exited yet.
        // Check the standard key output itself because default layer might also be transparent
        // and have delegated to defsrc handling.
        log::debug!("checking defsrc output");
        let kc = event.code.into();
        if self.cur_keys.contains(&kc)
            || self.unshifted_keys.contains(&kc)
            || self.unmodded_keys.contains(&kc)
        {
            if let Err(e) =
                write_key(&mut self.kbd_out, event.code, KeyValue::Repeat)
            {
                bail!("could not write key {e:?}");
            }
        }
        Ok(())
    }
}
