use super::*;

pub(crate) fn visit_nested_actions(
    action: &KanataAction,
    visit: &mut dyn FnMut(&KanataAction) -> Result<()>,
) -> Result<()> {
    match action {
        Action::HoldTap(HoldTapAction {
            tap,
            hold,
            timeout_action,
            ..
        }) => {
            visit(tap)?;
            visit(hold)?;
            visit(timeout_action)?;
        }
        Action::OneShot(OneShot { action: ac, .. }) => {
            visit(ac)?;
        }
        Action::MultipleActions(actions) => {
            for ac in actions.iter() {
                visit(ac)?;
            }
        }
        Action::TapDance(TapDance { actions, .. }) => {
            for ac in actions.iter() {
                visit(ac)?;
            }
        }
        Action::Fork(ForkConfig { left, right, .. }) => {
            visit(left)?;
            visit(right)?;
        }
        Action::Chords(ChordsGroup { chords, .. }) => {
            for (_, ac) in chords.iter() {
                visit(ac)?;
            }
        }
        Action::Switch(Switch { cases }) => {
            for case in cases.iter() {
                visit(case.1)?;
            }
        }
        ac @ Action::KeyCode(_)
        | ac @ Action::NoOp
        | ac @ Action::Custom(_)
        | ac @ Action::Trans
        | ac @ Action::MultipleKeyCodes(_)
        | ac @ Action::Repeat
        | ac @ Action::Layer(_)
        | ac @ Action::DefaultLayer(_)
        | ac @ Action::Sequence { .. }
        | ac @ Action::RepeatableSequence { .. }
        | ac @ Action::CancelSequences
        | ac @ Action::ReleaseState(_) => {
            visit(ac)?;
        }
    };
    Ok(())
}
