use super::*;

use crate::anyhow_expr;
use crate::bail;

pub(crate) fn parse_distance(expr: &SExpr, s: &ParserState, label: &str) -> Result<u16> {
    expr.atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|d| d.ok())
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be 1-30000"))
}

pub(crate) fn parse_mwheel(
    ac_params: &[SExpr],
    direction: MWheelDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "mwheel expects 2 parameters: <interval (ms)> <distance>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let interval = parse_non_zero_u16(&ac_params[0], s, "interval")?;
    let distance = parse_distance(&ac_params[1], s, "distance")?;
    custom(
        CustomAction::MWheel {
            direction,
            interval,
            distance,
            inertial_scroll_params: None,
        },
        &s.a,
    )
}

pub(crate) fn parse_mwheel_accel(
    ac_params: &[SExpr],
    direction: MWheelDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "mwheel-accel expects 4 float32 parameters:\n\
                           - initial velocity\n- maximum velocity\n\
                           - acceleration multiplier\n- deceleration multiplier";
    if ac_params.len() != 4 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let initial_velocity = parse_f32(&ac_params[0], s, "initial velocity", 1.0, 12000.0)?;
    let maximum_velocity = parse_f32(&ac_params[1], s, "maximum velocity", 1.0, 12000.0)?;
    let acceleration_multiplier =
        parse_f32(&ac_params[2], s, "acceleration multiplier", 1.0, 1000.0)?;
    let deceleration_multiplier =
        parse_f32(&ac_params[3], s, "deceleration multiplier", 0.0, 0.99)?;
    custom(
        CustomAction::MWheel {
            direction,
            interval: 16,
            distance: 1,
            inertial_scroll_params: Some(s.a.sref(MWheelInertial {
                initial_velocity,
                maximum_velocity,
                acceleration_multiplier,
                deceleration_multiplier,
            })),
        },
        &s.a,
    )
}

pub(crate) fn parse_move_mouse(
    ac_params: &[SExpr],
    direction: MoveDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "movemouse expects 2 parameters: <interval (ms)> <distance (px)>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let interval = parse_non_zero_u16(&ac_params[0], s, "interval")?;
    let distance = parse_distance(&ac_params[1], s, "distance")?;
    custom(
        CustomAction::MoveMouse {
            direction,
            interval,
            distance,
        },
        &s.a,
    )
}

pub(crate) fn parse_move_mouse_accel(
    ac_params: &[SExpr],
    direction: MoveDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 4 {
        bail!(
            "movemouse-accel expects four parameters, found {}\n<interval (ms)> <acceleration time (ms)> <min_distance> <max_distance>",
            ac_params.len()
        );
    }
    let interval = parse_non_zero_u16(&ac_params[0], s, "interval")?;
    let accel_time = parse_non_zero_u16(&ac_params[1], s, "acceleration time")?;
    let min_distance = parse_distance(&ac_params[2], s, "min distance")?;
    let max_distance = parse_distance(&ac_params[3], s, "max distance")?;
    if min_distance > max_distance {
        bail!("min distance should be less than max distance")
    }
    custom(
        CustomAction::MoveMouseAccel {
            direction,
            interval,
            accel_time,
            min_distance,
            max_distance,
        },
        &s.a,
    )
}

pub(crate) fn parse_move_mouse_speed(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 1 {
        bail!(
            "movemouse-speed expects one parameter, found {}\n<speed scaling % (1-65535)>",
            ac_params.len()
        );
    }
    let speed = parse_non_zero_u16(&ac_params[0], s, "speed scaling %")?;
    custom(CustomAction::MoveMouseSpeed { speed }, &s.a)
}

pub(crate) fn parse_set_mouse(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 2 {
        bail!(
            "movemouse-accel expects two parameters, found {}: <x> <y>",
            ac_params.len()
        );
    }
    let x = parse_u16(&ac_params[0], s, "x")?;
    let y = parse_u16(&ac_params[1], s, "y")?;
    custom(CustomAction::SetMouse { x, y }, &s.a)
}
