# Kanata WASM

Code to expose kanata functionality over WASM.

Prerequisites:

```
cargo install wasm-pack
```

You can run the command below to generate files for use in the browser:

```
wasm-pack build --target web
```

This will output files into `pkg/` which can be used for a website.
This has yet not been tested with targets other than web (e.g. node).

An example project using this code is the
[online kanata simulator](https://github.com/jtroo/jtroo.github.io).
