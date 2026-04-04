use super::*;

use crate::bail;

pub(crate) fn parse_caps_word(
    ac_params: &[SExpr],
    repress_behaviour: CapsWordRepressBehaviour,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "caps-word expects 1 param: <timeout>";
    if ac_params.len() != 1 {
        bail!("{ERR_STR}\nFound {} params instead of 1", ac_params.len());
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    custom(
        CustomAction::CapsWord(CapsWordCfg {
            repress_behaviour,
            keys_to_capitalize: &[
                KeyCode::A,
                KeyCode::B,
                KeyCode::C,
                KeyCode::D,
                KeyCode::E,
                KeyCode::F,
                KeyCode::G,
                KeyCode::H,
                KeyCode::I,
                KeyCode::J,
                KeyCode::K,
                KeyCode::L,
                KeyCode::M,
                KeyCode::N,
                KeyCode::O,
                KeyCode::P,
                KeyCode::Q,
                KeyCode::R,
                KeyCode::S,
                KeyCode::T,
                KeyCode::U,
                KeyCode::V,
                KeyCode::W,
                KeyCode::X,
                KeyCode::Y,
                KeyCode::Z,
                KeyCode::Minus,
            ],
            keys_nonterminal: &[
                KeyCode::Kb0,
                KeyCode::Kb1,
                KeyCode::Kb2,
                KeyCode::Kb3,
                KeyCode::Kb4,
                KeyCode::Kb5,
                KeyCode::Kb6,
                KeyCode::Kb7,
                KeyCode::Kb8,
                KeyCode::Kb9,
                KeyCode::Kp0,
                KeyCode::Kp1,
                KeyCode::Kp2,
                KeyCode::Kp3,
                KeyCode::Kp4,
                KeyCode::Kp5,
                KeyCode::Kp6,
                KeyCode::Kp7,
                KeyCode::Kp8,
                KeyCode::Kp9,
                KeyCode::BSpace,
                KeyCode::Delete,
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Left,
                KeyCode::Right,
            ],
            timeout,
        }),
        &s.a,
    )
}

pub(crate) fn parse_caps_word_custom(
    ac_params: &[SExpr],
    repress_behaviour: CapsWordRepressBehaviour,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "caps-word-custom expects 3 param: <timeout> <keys-to-capitalize> <extra-non-terminal-keys>";
    if ac_params.len() != 3 {
        bail!("{ERR_STR}\nFound {} params instead of 3", ac_params.len());
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    custom(
        CustomAction::CapsWord(CapsWordCfg {
            repress_behaviour,
            keys_to_capitalize: s.a.sref_vec(
                parse_key_list(&ac_params[1], s, "keys-to-capitalize")?
                    .into_iter()
                    .map(KeyCode::from)
                    .collect(),
            ),
            keys_nonterminal: s.a.sref_vec(
                parse_key_list(&ac_params[2], s, "extra-non-terminal-keys")?
                    .into_iter()
                    .map(KeyCode::from)
                    .collect(),
            ),
            timeout,
        }),
        &s.a,
    )
}
