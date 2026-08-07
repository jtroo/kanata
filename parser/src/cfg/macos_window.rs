use super::*;
use crate::{anyhow_expr, bail, bail_expr};

pub(crate) fn parse_macos_window(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!(
            "macos-window expects at least one layout preset or frame: left-half or (x y width height)"
        );
    }

    let mut layouts = Vec::with_capacity(ac_params.len());
    for layout in ac_params {
        layouts.push(parse_layout(layout, s)?);
    }

    custom(CustomAction::MacosWindow(s.a.sref_vec(layouts)), &s.a)
}

fn parse_layout(expr: &SExpr, s: &ParserState) -> Result<MacosWindowLayout> {
    if let Some(values) = expr.list(s.vars()) {
        return parse_frame(expr, values, s).map(MacosWindowLayout::Frame);
    }

    if let Some(atom) = expr.atom(s.vars()) {
        let atom = atom.trim_atom_quotes();
        return parse_preset(atom)
            .map(MacosWindowLayout::Preset)
            .ok_or_else(|| anyhow_expr!(expr, "unknown macos-window preset: {atom}"));
    }

    bail_expr!(
        expr,
        "macos-window layouts must be presets or frame lists: left-half or (x y width height)"
    );
}

fn parse_frame(expr: &SExpr, values: &[SExpr], s: &ParserState) -> Result<MacosWindowFrame> {
    if values.len() != 4 {
        bail_expr!(
            expr,
            "macos-window frames must have exactly four numbers: (x y width height)"
        );
    }
    let parsed = MacosWindowFrame {
        x: parse_basis_points(&values[0], s, "x")?,
        y: parse_basis_points(&values[1], s, "y")?,
        width: parse_basis_points(&values[2], s, "width")?,
        height: parse_basis_points(&values[3], s, "height")?,
    };
    if parsed.width <= 0 || parsed.height <= 0 {
        bail_expr!(expr, "macos-window width and height must be greater than 0");
    }
    Ok(parsed)
}

fn parse_preset(atom: &str) -> Option<MacosWindowPreset> {
    Some(match atom {
        "maximize" | "max" => MacosWindowPreset::Maximize,
        "almost-maximize" | "almost-max" | "almost" => MacosWindowPreset::AlmostMaximize,
        "left-half" | "left" => MacosWindowPreset::LeftHalf,
        "right-half" | "right" => MacosWindowPreset::RightHalf,
        "top-half" | "top" => MacosWindowPreset::TopHalf,
        "bottom-half" | "bottom" => MacosWindowPreset::BottomHalf,
        "center" | "centered" => MacosWindowPreset::Center,
        "first-third" | "left-third" => MacosWindowPreset::FirstThird,
        "center-third" | "middle-horizontal-third" => MacosWindowPreset::CenterThird,
        "last-third" | "right-third" => MacosWindowPreset::LastThird,
        "left-two-thirds" | "first-two-thirds" => MacosWindowPreset::LeftTwoThirds,
        "center-two-thirds" | "middle-two-thirds" => MacosWindowPreset::CenterTwoThirds,
        "right-two-thirds" | "last-two-thirds" => MacosWindowPreset::RightTwoThirds,
        "first-three-fourths"
        | "left-three-fourths"
        | "first-three-quarters"
        | "left-three-quarters" => MacosWindowPreset::FirstThreeFourths,
        "center-three-fourths"
        | "middle-three-fourths"
        | "center-three-quarters"
        | "middle-three-quarters" => MacosWindowPreset::CenterThreeFourths,
        "last-three-fourths"
        | "right-three-fourths"
        | "last-three-quarters"
        | "right-three-quarters" => MacosWindowPreset::LastThreeFourths,
        "top-third" => MacosWindowPreset::TopThird,
        "middle-third" | "vertical-middle-third" | "center-vertical-third" => {
            MacosWindowPreset::MiddleThird
        }
        "bottom-third" => MacosWindowPreset::BottomThird,
        "top-two-thirds" => MacosWindowPreset::TopTwoThirds,
        "bottom-two-thirds" => MacosWindowPreset::BottomTwoThirds,
        "top-center-two-thirds" | "top-middle-two-thirds" => MacosWindowPreset::TopCenterTwoThirds,
        "bottom-center-two-thirds" | "bottom-middle-two-thirds" => {
            MacosWindowPreset::BottomCenterTwoThirds
        }
        "top-first-fourth" | "top-first-quarter" => MacosWindowPreset::TopFirstFourth,
        "top-second-fourth" | "top-second-quarter" => MacosWindowPreset::TopSecondFourth,
        "top-third-fourth" | "top-third-quarter" => MacosWindowPreset::TopThirdFourth,
        "top-last-fourth" | "top-last-quarter" => MacosWindowPreset::TopLastFourth,
        "top-three-fourths" | "top-three-quarters" => MacosWindowPreset::TopThreeFourths,
        "bottom-three-fourths" | "bottom-three-quarters" => MacosWindowPreset::BottomThreeFourths,
        "first-fourth" | "left-fourth" | "first-quarter" | "left-quarter" => {
            MacosWindowPreset::FirstFourth
        }
        "second-fourth" | "second-quarter" => MacosWindowPreset::SecondFourth,
        "third-fourth" | "third-quarter" => MacosWindowPreset::ThirdFourth,
        "last-fourth" | "right-fourth" | "last-quarter" | "right-quarter" => {
            MacosWindowPreset::LastFourth
        }
        "top-left-quarter" | "top-left-fourth" => MacosWindowPreset::TopLeftQuarter,
        "top-right-quarter" | "top-right-fourth" => MacosWindowPreset::TopRightQuarter,
        "bottom-left-quarter" | "bottom-left-fourth" => MacosWindowPreset::BottomLeftQuarter,
        "bottom-right-quarter" | "bottom-right-fourth" => MacosWindowPreset::BottomRightQuarter,
        "top-left-sixth" => MacosWindowPreset::TopLeftSixth,
        "top-center-sixth" | "top-middle-sixth" => MacosWindowPreset::TopCenterSixth,
        "top-right-sixth" => MacosWindowPreset::TopRightSixth,
        "bottom-left-sixth" => MacosWindowPreset::BottomLeftSixth,
        "bottom-center-sixth" | "bottom-middle-sixth" => MacosWindowPreset::BottomCenterSixth,
        "bottom-right-sixth" => MacosWindowPreset::BottomRightSixth,
        "move-left" => MacosWindowPreset::MoveLeft,
        "move-right" => MacosWindowPreset::MoveRight,
        "move-top" | "move-up" => MacosWindowPreset::MoveTop,
        "move-bottom" | "move-down" => MacosWindowPreset::MoveBottom,
        _ => return None,
    })
}

fn parse_basis_points(expr: &SExpr, s: &ParserState, label: &str) -> Result<i32> {
    let Some(atom) = expr.atom(s.vars()) else {
        bail_expr!(expr, "macos-window {label} must be a number");
    };
    let atom = atom.trim_atom_quotes();
    atom.parse::<i32>().map_err(|_| {
        anyhow_expr!(
            expr,
            "macos-window {label} must be a signed integer basis-point value"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_canonical_preset_name_parses() {
        use MacosWindowPreset::*;

        let cases = [
            ("maximize", Maximize),
            ("almost-maximize", AlmostMaximize),
            ("left-half", LeftHalf),
            ("right-half", RightHalf),
            ("top-half", TopHalf),
            ("bottom-half", BottomHalf),
            ("center", Center),
            ("first-third", FirstThird),
            ("center-third", CenterThird),
            ("last-third", LastThird),
            ("left-two-thirds", LeftTwoThirds),
            ("center-two-thirds", CenterTwoThirds),
            ("right-two-thirds", RightTwoThirds),
            ("first-three-fourths", FirstThreeFourths),
            ("center-three-fourths", CenterThreeFourths),
            ("last-three-fourths", LastThreeFourths),
            ("top-third", TopThird),
            ("middle-third", MiddleThird),
            ("bottom-third", BottomThird),
            ("top-two-thirds", TopTwoThirds),
            ("bottom-two-thirds", BottomTwoThirds),
            ("top-center-two-thirds", TopCenterTwoThirds),
            ("bottom-center-two-thirds", BottomCenterTwoThirds),
            ("top-first-fourth", TopFirstFourth),
            ("top-second-fourth", TopSecondFourth),
            ("top-third-fourth", TopThirdFourth),
            ("top-last-fourth", TopLastFourth),
            ("top-three-fourths", TopThreeFourths),
            ("bottom-three-fourths", BottomThreeFourths),
            ("first-fourth", FirstFourth),
            ("second-fourth", SecondFourth),
            ("third-fourth", ThirdFourth),
            ("last-fourth", LastFourth),
            ("top-left-quarter", TopLeftQuarter),
            ("top-right-quarter", TopRightQuarter),
            ("bottom-left-quarter", BottomLeftQuarter),
            ("bottom-right-quarter", BottomRightQuarter),
            ("top-left-sixth", TopLeftSixth),
            ("top-center-sixth", TopCenterSixth),
            ("top-right-sixth", TopRightSixth),
            ("bottom-left-sixth", BottomLeftSixth),
            ("bottom-center-sixth", BottomCenterSixth),
            ("bottom-right-sixth", BottomRightSixth),
            ("move-left", MoveLeft),
            ("move-right", MoveRight),
            ("move-top", MoveTop),
            ("move-bottom", MoveBottom),
        ];

        for (name, expected) in cases {
            assert_eq!(parse_preset(name), Some(expected), "{name}");
        }
        assert_eq!(cases.len(), 47);
    }
}
