# Comparison with kmonad

The kmonad project is the closest alternative for this project. It is also more
mature and has had many more contributions than kanata.

## Benefits of kmonad over kanata

- MacOS support
- Different features

## Why I built and use kanata

Limitations that don't affect me:

- I only use Windows and Linux (PRs for MacOS support are welcome though!)
- My personal layout in QMK is fully replicable with the
  [keyberon library's features](https://github.com/TeXitoi/keyberon/blob/master/src/action.rs)

Why I don't use kmonad:

- [Double-tapping a tap-hold key](https://github.com/kmonad/kmonad/issues/163) does not behave
  [how I want it to](https://docs.qmk.fm/#/tap_hold?id=tapping-force-hold)
- Some key sequences with tap-hold keys [don't behave how I want](https://github.com/kmonad/kmonad/issues/466):
  - `(press lsft) (press a) (release lsft) (release a)` (a is a tap-hold key)
  - The above outputs `a` in kmonad, but I want it to output `A`
- kanata supports sending mouse buttons but [kmonad does not](https://github.com/kmonad/kmonad/issues/150)

The issues listed are all fixable in kmonad and I hope they are one day! For me
though, I don't know Haskell well enough to poke around the kmonad codebase and
attempt fixing these. That's why I instead built kanata based off of the
excellent work that had already gone into the
[keyberon](https://github.com/TeXitoi/keyberon),
[ktrl](https://github.com/ItayGarin/ktrl), and
[kbremap](https://github.com/timokroeger/kbremap) projects.

If you want to see which features are supported in kanata, the
[sample configuration](../cfg_samples/kanata.kbd) and features list in the
[README](../README.md#features) should hopefully provide insight.

I dogfood kanata myself and it works great for my use cases. If you don't use
any of the missing features from kmonad or are willing to part with them (or
implement them), give kanata a try!
