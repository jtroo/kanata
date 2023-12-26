# Comparison with kmonad

The kmonad project is the closest alternative for this project.

## Benefits of kmonad over kanata

- ~MacOS support~ (this is implemented now)
- Different features

## Why I built and use kanata

- [Double-tapping a tap-hold key](https://github.com/kmonad/kmonad/issues/163) did not behave
  [how I want it to](https://docs.qmk.fm/#/tap_hold?id=tapping-force-hold)
- Some key sequences with tap-hold keys [didn't behave how I want](https://github.com/kmonad/kmonad/issues/466):
  - `(press lsft) (press a) (release lsft) (release a)` (a is a tap-hold key)
  - The above outputs `a` in kmonad, but I want it to output `A`
- kmonad was missing [mouse buttons](https://github.com/kmonad/kmonad/issues/150)

The issues listed are all fixable in kmonad and I hope they are one day! For me
though, I didn't and still don't know Haskell well enough to contribute to
kmonad. That's why I instead built kanata based off of the excellent work that
had already gone into the
[keyberon](https://github.com/TeXitoi/keyberon),
[ktrl](https://github.com/ItayGarin/ktrl), and
[kbremap](https://github.com/timokroeger/kbremap) projects.

If you want to see the features that kanata offers, the
[configuration guide](./config.adoc) is a good starting point.

I dogfood kanata myself and it works great for my use cases. Though kanata is a
younger project than kmonad, it now has more features. If you give kanata a
try, feel free to ask for help in an issue or discussion, or let me know how it
went 🙂.
