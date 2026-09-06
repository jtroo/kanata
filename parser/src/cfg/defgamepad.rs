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

// ------------------------------------------------------------------- items

/// Refuse a second `what`, at every level of the grammar.
///
/// Letting the later one quietly win makes a typo look like a working
/// configuration. Mouse and scroll are distinct projections; repeating either
/// one is still almost certainly a typo.
fn once(seen: &mut Vec<String>, at: &SExpr, what: impl Into<String>) -> Result<()> {
    let what = what.into();
    if seen.contains(&what) {
        bail_expr!(at, "duplicate {what}");
    }
    seen.push(what);
    Ok(())
}

/// Split `(<item> left|right ...)` into the side and the remaining options.
fn sided<'a>(
    rest: &'a [SExpr],
    at: &SExpr,
    vars: &HashMap<String, SExpr>,
    what: &str,
) -> Result<(Side, &'a [SExpr])> {
    let Some((side, options)) = rest.split_first() else {
        bail_expr!(
            at,
            "{what} needs a side: ({what} left ...) or ({what} right ...)"
        );
    };
    Ok((one_of(side, vars, "side", &SIDES)?, options))
}

/// Parse the projections of a stick or the d-pad.
///
/// A control may take both halves: a stick can drive the pointer *and* press a
/// key at full deflection, because the threshold and the displacement read the
/// same value without disturbing each other.
fn projection(
    control: Directional,
    mut projection: Projection,
    args: &[SExpr],
    at: &SExpr,
    vars: &HashMap<String, SExpr>,
) -> Result<Projection> {
    if args.is_empty() {
        bail_expr!(at, "the {} needs one of {PROJECTIONS}", control.as_str());
    }
    // `off` is the one bare-atom form, and it is exclusive. `(stick left
    // (digital) off)` reads to a user exactly like `(stick left off
    // (digital))` but silently produces the opposite, so neither is allowed.
    let is_off = |arg: &SExpr| arg.atom(Some(vars)).map(str::trim_atom_quotes) == Some("off");
    if args.len() > 1 {
        if let Some(off) = args.iter().find(|arg| is_off(arg)) {
            bail_expr!(
                off,
                "`off` is the whole projection: a control that does nothing has nothing \
                 else to declare"
            );
        }
    }

    let mut seen: Vec<String> = Vec::new();
    for arg in args {
        if is_off(arg) {
            return Ok(Projection::OFF);
        }
        let (keyword, options) = keyed(arg, vars, PROJECTIONS)?;
        match motion_kind(keyword) {
            Some(kind) => {
                once(&mut seen, arg, format!("({} ...)", motion_name(kind)))?;
                projection
                    .motions
                    .set(kind, motion(kind, Axes::Two, options, vars)?);
            }
            None if keyword == "digital" => {
                once(&mut seen, arg, "(digital ...)")?;
                projection.digital = Some(digital(control, options, vars)?);
            }
            None => bail_expr!(
                arg,
                "unknown projection: {keyword}\na stick or d-pad takes {PROJECTIONS}"
            ),
        }
    }
    Ok(projection)
}

fn digital(control: Directional, args: &[SExpr], vars: &HashMap<String, SExpr>) -> Result<Digital> {
    let mut digital = Digital::default();
    let mut seen: Vec<String> = Vec::new();
    for arg in args {
        let (keyword, values) = keyed(arg, vars, "a digital option such as (mode 8way)")?;
        match keyword {
            "mode" | "socd" | "threshold" => once(&mut seen, arg, format!("({keyword} ...)"))?,
            _ => bail_expr!(
                arg,
                "unknown digital option: {keyword}\nvalid options: mode, socd, threshold"
            ),
        }
        match keyword {
            "mode" => digital.mode = one_of(value(values, arg, keyword)?, vars, "mode", &MODES)?,
            // Only a control with independent contacts can assert an opposed
            // pair, and a stick reports each axis as one signed number.
            // Accepting this there would be a knob that never fires.
            "socd" if control.side().is_some() => bail_expr!(
                arg,
                "socd belongs to the d-pad: a stick reads each axis as one number, so it \
                 can never report two opposite directions at once.\n\
                 Use (dpad (digital (socd ...)))."
            ),
            "socd" => {
                digital.socd = one_of(value(values, arg, keyword)?, vars, "socd mode", &SOCDS)?
            }
            _ if control == Directional::Dpad => bail_expr!(
                arg,
                "a d-pad has no {keyword} threshold: its contacts are digital before \
                 they reach kanata, so there is no analog value to compare"
            ),
            "threshold" => {
                digital.threshold = unit(value(values, arg, keyword)?, vars, keyword)?;
            }
            _ => unreachable!("validated above"),
        }
    }
    Ok(digital)
}

fn trigger(args: &[SExpr], vars: &HashMap<String, SExpr>) -> Result<Trigger> {
    let mut trigger = Trigger::default();
    let mut seen: Vec<String> = Vec::new();
    for arg in args {
        let (keyword, values) = keyed(arg, vars, "a trigger option such as (threshold 0.3)")?;
        match (keyword, motion_kind(keyword)) {
            (_, Some(kind)) => {
                once(&mut seen, arg, format!("({} ...)", motion_name(kind)))?;
                // The direction has to be an atom, so a leading option is a
                // missing direction rather than an unreadable one.
                let named = values
                    .split_first()
                    .filter(|(dir, _)| dir.atom(Some(vars)).is_some());
                let Some((dir, rest)) = named else {
                    bail_expr!(
                        arg,
                        "({keyword} ...) on a trigger needs a direction to push: a trigger is \
                         one number, not a vector, e.g. ({keyword} down (speed 30))"
                    );
                };
                let dir = one_of(dir, vars, "direction", &PUSH)?;
                trigger.motions.set(
                    kind,
                    TriggerMotion {
                        direction: dir,
                        motion: motion(kind, Axes::One, rest, vars)?,
                    },
                );
            }
            ("threshold", _) => {
                once(&mut seen, arg, format!("({keyword} ...)"))?;
                trigger.threshold = unit(value(values, arg, keyword)?, vars, keyword)?;
            }
            _ => bail_expr!(
                arg,
                "unknown trigger option: {keyword}\n\
                 valid options: threshold, mouse, scroll"
            ),
        }
    }
    Ok(trigger)
}

/// How many axes the control being projected has.
///
/// A trigger is a single number that already names the direction it pushes, so
/// `invert-x` / `invert-y` there would be a second knob for the same thing --
/// and, since the trigger path never reads `Motion::invert`, one that did
/// nothing at all.
#[derive(Clone, Copy, PartialEq)]
enum Axes {
    One,
    Two,
}

fn motion(
    kind: MotionKind,
    axes: Axes,
    args: &[SExpr],
    vars: &HashMap<String, SExpr>,
) -> Result<Motion> {
    let mut motion = Motion::of(kind);
    let mut seen: Vec<String> = Vec::new();
    for arg in args {
        let (keyword, values) = keyed(arg, vars, "a motion option such as (speed 1200)")?;
        match keyword {
            "invert-x" | "invert-y" if axes == Axes::One => bail_expr!(
                arg,
                "a trigger has no {keyword}: it is one number, and the direction after \
                 ({} ...) already says which way it pushes",
                motion_name(kind)
            ),
            "deadzone" | "speed" | "curve" | "invert-x" | "invert-y" => {
                once(&mut seen, arg, format!("({keyword} ...)"))?
            }
            _ => bail_expr!(
                arg,
                "unknown {} option: {keyword}\n\
                 valid options: deadzone, speed, curve, invert-x, invert-y",
                motion_name(kind)
            ),
        }
        // The value is read after the name is known to be one we handle, so
        // that an unknown option is reported as one rather than as an arity
        // error about a name that means nothing here.
        let value = value(values, arg, keyword)?;
        match keyword {
            "deadzone" => motion.deadzone = unit(value, vars, keyword)?,
            "speed" => motion.speed = number(value, vars, keyword, Motion::MAX_SPEED)?,
            "curve" => motion.curve = one_of(value, vars, "curve", &CURVES)?,
            "invert-x" => motion.invert.x = sign(one_of(value, vars, keyword, &BOOLS)?),
            _ => motion.invert.y = sign(one_of(value, vars, keyword, &BOOLS)?),
        }
    }
    Ok(motion)
}

fn slot(
    rest: &[SExpr],
    at: &SExpr,
    vars: &HashMap<String, SExpr>,
    cfg: &mut GamepadConfig,
) -> Result<()> {
    let [index_expr, code_expr] = rest else {
        bail_expr!(
            at,
            "button-slot takes a slot index and a backend code, e.g. (button-slot 0 0x2c0)\n\
             kanata logs the backend code of every control it cannot name"
        );
    };
    let index: u8 = atom(index_expr, vars, "slot index")?
        .parse()
        .ok()
        .filter(|index| *index < PadCode::SLOTS)
        .ok_or_else(|| {
            anyhow_expr!(
                index_expr,
                "slot index must be 0-{}; that is how many pad-button-N controls exist",
                PadCode::SLOTS - 1
            )
        })?;
    let text = atom(code_expr, vars, "backend code")?;
    let code = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
    .ok_or_else(|| {
        anyhow_expr!(
            code_expr,
            "backend code must be a number or 0x-prefixed hex"
        )
    })?;

    if cfg.slots[index as usize].is_some() {
        bail_expr!(index_expr, "duplicate button-slot: pad-button-{index}");
    }
    if let Some(taken) = cfg.slots.iter().position(|bound| *bound == Some(code)) {
        bail_expr!(
            code_expr,
            "backend code {code} is already bound to pad-button-{taken}"
        );
    }
    cfg.slots[index as usize] = Some(code);
    Ok(())
}
