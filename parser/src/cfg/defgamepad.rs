//! Parsing for the `defgamepad` declaration, which says what physical
//! controls exist and how they project. It never names an output key.
//!
//! ```lisp
//! (defgamepad
//!   (stick left  (digital (mode 4way)))
//!   (stick right (mouse (deadzone 0.12) (speed 1200)))
//!   (dpad        (digital (mode 8way) (socd neutral)))
//!   (trigger right (threshold 0.30) (scroll down (speed 30))))
//! ```
//!
//! The three directional controls take the same options because they project
//! the same way, so there is one grammar rather than one per control.

use super::*;
use crate::gamepad::{
    Cardinal, Curve, Digital, DirMode, Directional, GamepadConfig, Motion, MotionKind, PadCode,
    PadControl, Projection, Side, Socd, Trigger, TriggerMotion, Unit,
};
use crate::{anyhow_expr, bail, bail_expr};

use std::f32::consts::SQRT_2;
use std::num::NonZeroU8;

const ITEM: &str = "a defgamepad item such as (stick left (digital))";
const PROJECTIONS: &str = "(digital ...), (mouse ...), (scroll ...), or the bare atom `off`";

const SIDES: [(&str, Side); 2] = [("left", Side::Left), ("right", Side::Right)];
const MODES: [(&str, DirMode); 4] = [
    ("4way", DirMode::FourWay),
    ("four-way", DirMode::FourWay),
    ("8way", DirMode::EightWay),
    ("eight-way", DirMode::EightWay),
];
const SOCDS: [(&str, Socd); 6] = [
    ("off", Socd::Off),
    ("neutral", Socd::Neutral),
    ("last", Socd::Last),
    ("first", Socd::First),
    ("positive", Socd::Positive),
    ("negative", Socd::Negative),
];
const CURVES: [(&str, Curve); 3] = [
    ("linear", Curve::Linear),
    ("quadratic", Curve::Quadratic),
    ("cubic", Curve::Cubic),
];
const BOOLS: [(&str, bool); 4] = [
    ("yes", true),
    ("true", true),
    ("no", false),
    ("false", false),
];
/// A trigger is one number, so a continuous projection has to be told which
/// way to push. Cardinals only: a diagonal would run 41% fast unless it were
/// normalized, and naming one is asking for a stick anyway.
const PUSH: [(&str, Cardinal); 4] = [
    ("up", Cardinal::Up),
    ("down", Cardinal::Down),
    ("left", Cardinal::Left),
    ("right", Cardinal::Right),
];

pub fn parse_defgamepad(expr: &[SExpr], vars: &HashMap<String, SExpr>) -> Result<GamepadConfig> {
    let mut cfg = GamepadConfig::default();
    let mut seen: Vec<String> = Vec::new();

    for item in check_first_expr(expr.iter(), "defgamepad")? {
        let (keyword, rest) = keyed(item, vars, ITEM)?;
        match keyword {
            "device" => {
                once(&mut seen, item, "(device ...)")?;
                cfg.device = Some(device_id(value(rest, item, keyword)?, vars)?);
            }
            "stick" => {
                let (side, rest) = sided(rest, item, vars, keyword)?;
                once(&mut seen, item, format!("(stick {} ...)", side.as_str()))?;
                let control = side.stick();
                *cfg.projection_mut(control) =
                    projection(control, Projection::OFF, rest, item, vars)?;
            }
            "dpad" => {
                once(&mut seen, item, "(dpad ...)")?;
                // Starts from the default rather than from nothing: a d-pad
                // presses keys with no declaration at all, so `(dpad (scroll
                // ...))` must add the wheel without taking that away.
                let control = Directional::Dpad;
                let base = *cfg.projection(control);
                *cfg.projection_mut(control) = projection(control, base, rest, item, vars)?;
            }
            "trigger" => {
                let (side, rest) = sided(rest, item, vars, keyword)?;
                once(&mut seen, item, format!("(trigger {} ...)", side.as_str()))?;
                *cfg.trigger_mut(side) = trigger(rest, vars)?;
            }
            "button-slot" => slot(rest, item, vars, &mut cfg)?,
            _ => bail_expr!(
                item,
                "unknown defgamepad item: {keyword}\n\
                 valid items: device, stick, dpad, trigger, button-slot"
            ),
        }
    }
    Ok(cfg)
}

/// Check a parsed `defgamepad` against the rest of the configuration.
///
/// The failures caught here are the ones that otherwise present as "kanata
/// runs, my controller does nothing": a direction mapped in `defsrc` with no
/// projection that could ever produce it, or a device ID that matches no
/// `definputdevices` entry.
pub fn validate_gamepad(
    cfg: &GamepadConfig,
    input_devices: Option<&[(NonZeroU8, InputDeviceMatcher)]>,
    mapped_keys: &MappedKeys,
) -> Result<()> {
    let unknown = cfg.device.filter(|device| {
        !input_devices.is_some_and(|devices| devices.iter().any(|(id, _)| id == device))
    });
    if let Some(device) = unknown {
        bail!(
            "defgamepad refers to (device {device}) but definputdevices has no entry \
             with that ID.\n\
             Either add one, or remove the (device ...) line to accept every connected \
             controller."
        );
    }

    for osc in mapped_keys.iter() {
        let Some(control) = PadCode::from_os_code(*osc).map(PadCode::control) else {
            continue;
        };
        match control {
            PadControl::Direction(control, dir) => {
                let name = control.as_str();
                let Some(digital) = cfg.projection(control).digital else {
                    bail!(
                        "defsrc maps {osc}, but the {name} has no digital projection.\n{}",
                        hint(cfg, control, "(digital)")
                    );
                };
                if dir.is_diagonal() {
                    // Four-way presses two cardinals for a diagonal and never
                    // the diagonal control itself. Eight-way still presses a
                    // cardinal for a straight push, so there every direction
                    // is reachable.
                    if digital.mode == DirMode::FourWay {
                        bail!(
                            "defsrc maps {osc}, but the {name} is in 4way mode, which never \
                             presses a diagonal control.\n{}\n\
                             Or map the cardinal controls instead.",
                            hint(cfg, control, "(digital (mode 8way))")
                        );
                    }
                    // Both components have to clear the threshold at once, so
                    // a diagonal needs `threshold * sqrt(2)` of deflection. Past
                    // full deflection only a square-gated stick can reach one,
                    // and this is otherwise another silent "my controller does
                    // nothing".
                    let threshold = digital.threshold.get();
                    if control.side().is_some() && threshold * SQRT_2 > 1.0 {
                        log::warn!(
                            "defsrc maps {osc}, but the {name} presses at {threshold:.2}, and \
                             holding two axes there at once needs {:.2} of deflection. Most \
                             sticks have a circular gate and cannot reach that; lower \
                             (threshold ...) to {:.2} or below.",
                            threshold * SQRT_2,
                            1.0 / SQRT_2
                        );
                    }
                }
            }
            PadControl::Slot(index) if cfg.slots[index as usize].is_none() => bail!(
                "defsrc maps {osc}, but no (button-slot {index} ...) binds it to a control.\n\
                 Press the control you want and check the log for its backend code, then \
                 add (button-slot {index} <code>)."
            ),
            PadControl::Button(_) | PadControl::Slot(_) => {}
        }
    }
    Ok(())
}

/// What to tell the user to write.
///
/// A control that already has an item cannot be given a second one -- that is
/// a duplicate -- so once it has any projection the hint has to say "add this
/// to the item you have" rather than offering a line that would be rejected.
fn hint(cfg: &GamepadConfig, control: Directional, inner: &str) -> String {
    let item = match control.side() {
        Some(side) => format!("(stick {} ", side.as_str()),
        None => "(dpad ".to_string(),
    };
    match *cfg.projection(control) == Projection::OFF {
        true => format!("Add {item}{inner}) to defgamepad."),
        false => format!("Add {inner} to the {item}...) you already have."),
    }
}

