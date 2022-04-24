# Comparison with kmonad

The kmonad project is the closest alternative for this project. It is also more
mature and has had many more contributions than kanata.

## Benefits of kmonad over kanata

- Way more features
- MacOS support

## Why I built and use kanata

- I only use Windows and Linux (PRs for MacOS support are welcome though!)
- My personal layout in QMK is fully replicable with the
  [keyberon library's features](https://github.com/TeXitoi/keyberon/blob/master/src/action.rs)
  (barring mouse functionality, which is also missing in kmonad for now)
- Key repeating in kmonad [doesn't work in Windows](https://github.com/kmonad/kmonad/issues/82)
  in the master branch
  - In the keycode-refactor branch, key repeat happens too slowly and is
    different from the speed that the OS would natively repeat the key
- [Double-tapping a tap-hold key](https://github.com/kmonad/kmonad/issues/163) does not behave
  [how I want it to](https://docs.qmk.fm/#/tap_hold?id=tapping-force-hold)
- Some key sequences with tap-hold keys [don't behave how I want](https://github.com/kmonad/kmonad/issues/466):
  - `(press lsft) (press a) (release lsft) (release a)` (a is a tap hold key)
  - The above outputs `a` in kmonad, but I want it to output `A`

These issues are all fixable in kmonad and I hope they are one day! For me
though, I don't know Haskell well enough to poke around the kmonad codebase and
attempt fixing these. That's why I instead built kanata in about a week, based
off of the excellent work that had already gone into
[keyberon](https://github.com/TeXitoi/keyberon),
[ktrl](https://github.com/ItayGarin/ktrl), and
[kbremap](https://github.com/timokroeger/kbremap) projects.

I already dogfood kanata when using my laptop keyboard and it works great for
my use cases. If you don't use any of the missing features from kmonad or are
willing to part with them (or implement them), give kanata a try!
