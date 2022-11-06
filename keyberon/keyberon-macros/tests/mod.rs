extern crate keyberon_macros;
use keyberon::action::{k, l, m, Action, Action::*, HoldTapConfig, HoldTapAction};
use keyberon::key_code::KeyCode::*;
use keyberon::layout::*;
use keyberon_macros::layout;

#[test]
fn test_layout_equality() {
    macro_rules! s {
        ($k:expr) => {
            m(&[LShift, $k].as_slice())
        };
    }

    static S_ENTER: Action = Action::HoldTap(&HoldTapAction {
        timeout: 280,
        hold: Action::KeyCode(RShift),
        tap: Action::KeyCode(Enter),
        config: HoldTapConfig::PermissiveHold,
        tap_hold_interval: 0,
    });

    #[rustfmt::skip]
    pub static LAYERS_OLD: Layers<12, 4, 2> = [
        [
            [k(Tab),    k(Q), k(W), k(E), k(R), k(T),   k(Y), k(U), k(I),     k(O),   k(P),      k(BSpace)],
            [k(LCtrl),  k(A), k(S), k(D), k(F), k(G),   k(H), k(J), k(K),     k(L),   k(SColon), k(Quote) ],
            [k(LShift), k(Z), k(X), k(C), k(V), k(B),   k(N), k(M), k(Comma), k(Dot), k(Slash),  k(Escape)],
            [NoOp, NoOp, k(LGui), l(1), k(Space), k(Escape),   k(BSpace), S_ENTER, l(1), k(RAlt), NoOp, NoOp],
        ],
        [
            [k(Tab),    k(Kb1), k(Kb2), k(Kb3), k(Kb4), k(Kb5),   k(Kb6),  k(Kb7),  k(Kb8), k(Kb9), k(Kb0), k(BSpace)],
            [k(LCtrl),  s!(Kb1), s!(Kb2), s!(Kb3), s!(Kb4), s!(Kb5),   s!(Kb6), s!(Kb7), s!(Kb8),  s!(Kb9), s!(Kb0), MultipleActions(&[k(LCtrl), k(Grave)].as_slice())],
            [k(LShift), NoOp, NoOp, NoOp, NoOp, NoOp,   k(Left), k(Down), k(Up), k(Right), NoOp, s!(Grave)],
            [NoOp, NoOp, k(LGui), Trans, Trans, Trans,   Trans, Trans, Trans, k(RAlt), NoOp, NoOp],
        ],
    ];

    pub static LAYERS: Layers<12, 4, 2> = layout! {
        {
            [ Tab    Q W E R T   Y U I O P BSpace ]
            [ LCtrl  A S D F G   H J K L ; Quote  ]
            [ LShift Z X C V B   N M , . / Escape ]
            [ n n LGui (1) Space Escape   BSpace {S_ENTER} (1) RAlt n n ]
        }
        {
            [ Tab    1 2 3 4 5   6 7 8 9 0 BSpace ]
            [ LCtrl  ! @ # $ %   ^ & * '(' ')' [LCtrl '`'] ]
            [ LShift n n n n n   Left Down Up Right n ~ ]
            [   n n LGui t t t   t t t RAlt n n ]
        }
    };

    assert_eq!(LAYERS, LAYERS_OLD);
    use std::mem::size_of_val;
    assert_eq!(size_of_val(&LAYERS), size_of_val(&LAYERS_OLD))
}

#[test]
fn test_nesting() {
    static A: Layers<2, 1, 1> = layout! {
        {
            [{k(D)} [(5) [C {k(D)}]]]
        }
    };
    static B: Layers<2, 1, 1> = [[[
        k(D),
        Action::MultipleActions(&[Action::Layer(5), Action::MultipleActions(&[k(C), k(D)].as_slice())].as_slice()),
    ]]];
    assert_eq!(A, B);
}

#[test]
fn test_layer_switch() {
    static A: Layers<5, 1, 1> = layout! {
        {
            [(0xa) (0b0110) (b'a' as usize) (1 + 8 & 32) ([4,5][0])]
        }
    };
    static B: Layers<5, 1, 1> = [[[
        Action::Layer(0xa),
        Action::Layer(6),
        Action::Layer(b'a' as usize),
        Action::Layer(1 + 8 & 32),
        Action::Layer([4, 5][0]),
    ]]];
    assert_eq!(A, B);
}

#[test]
fn test_escapes() {
    static A: Layers<2, 1, 1> = layout! {
        {
            ['\\' '\'']
        }
    };
    static B: Layers<2, 1, 1> = [[[k(Bslash), k(Quote)]]];
    assert_eq!(A, B);
}

#[test]
fn test_keycode_group_comma() {
    static A: Layers<3, 1, 1> = layout! {
        {
            [ C [D E] F ]
        }
    };
    static B: Layers<3, 1, 1> = [[[k(C), Action::MultipleActions(&[k(D), k(E)].as_slice()), k(F)]]];
    assert_eq!(A, B);
}
