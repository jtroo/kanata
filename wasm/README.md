# Kanata WASM

Code to expose kanata functionality over WASM.

Prerequisites:

- Install the wasm32-unknown-unknown rustc target
- Install wasm-bindgen: `cargo install wasm-bindgen-cli`
- Install wasm-opt: `cargo install wasm-opt`

You can run the command below to generate files for use in the browser:

```
wasm-pack build --target web
```

- Either install `just` and run `just wasm-build <output_directory>
  or execute the commands for the recipe (see `justfile` in the repository).

This will output files into `pkg/` which can be used for a website.
This has yet not been tested with targets other than web (e.g. node).

An example project using this code is the
[online kanata simulator](https://github.com/jtroo/jtroo.github.io).

The simulator files can be served by any webserver,
such as the default Python webserver:

```
python -m http.server <tcp_port>
```
