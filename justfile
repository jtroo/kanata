set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Build the release binaries for Linux and put the binaries+cfg in the output directory
build_release_linux output_dir:
  cargo build --release
  cp target/release/kanata "{{output_dir}}/kanata"
  strip "{{output_dir}}/kanata"
  cargo build --release --features cmd
  cp target/release/kanata "{{output_dir}}/kanata_cmd_allowed"
  strip "{{output_dir}}/kanata_cmd_allowed"
  cp cfg_samples/kanata.kbd "{{output_dir}}"

# Build the release binaries for Windows and put the binaries+cfg in the output directory.
build_release_windows output_dir:
  cargo build --release --no-default-features --features tcp_server,win_manifest; cp target/release/kanata.exe "{{output_dir}}\kanata_legacy_output.exe"
  cargo build --release --features win_manifest,interception_driver; cp target/release/kanata.exe "{{output_dir}}\kanata_wintercept.exe"
  cargo build --release --features win_manifest,win_sendinput_send_scancodes; cp target/release/kanata.exe "{{output_dir}}\kanata.exe"
  cargo build --release --features win_manifest,win_sendinput_send_scancodes,win_llhook_read_scancodes; cp target/release/kanata.exe "{{output_dir}}\kanata_winIOv2.exe"
  cargo build --release --features win_manifest,cmd,win_sendinput_send_scancodes; cp target/release/kanata.exe "{{output_dir}}\kanata_cmd_allowed.exe"
  cargo build --release --features win_manifest,cmd,interception_driver; cp target/release/kanata.exe "{{output_dir}}\kanata_wintercept_cmd_allowed.exe"
  cargo build --release --features passthru_ahk --package=simulated_passthru; cp target/release/kanata_passthru.dll "{{output_dir}}\kanata_passthru.dll"
  cargo build --release --features win_manifest,gui    ; cp target/release/kanata.exe "{{output_dir}}\kanata_gui.exe"
  cargo build --release --features win_manifest,gui,cmd; cp target/release/kanata.exe "{{output_dir}}\kanata_gui_cmd_allowed.exe"
  cargo build --release --features win_manifest,gui,interception_driver    ; cp target/release/kanata.exe "{{output_dir}}\kanata_gui_wintercept.exe"
  cargo build --release --features win_manifest,gui,cmd,interception_driver; cp target/release/kanata.exe "{{output_dir}}\kanata_gui_wintercept_cmd_allowed.exe"
  cp cfg_samples/kanata.kbd "{{output_dir}}"

# Generate the sha256sums for all files in the output directory
sha256sums output_dir:
  rm -f {{output_dir}}/sha256sums
  cd {{output_dir}}; sha256sum * > sha256sums

test:
  cargo test -p kanata -p kanata-parser -p kanata-keyberon -- --nocapture
  cargo test --features=simulated_output sim_tests
  cargo test --features=simulated_output -- must_be_single_threaded --ignored --test-threads=1
  cargo clippy --all

fmt:
  cargo fmt --all

guic:
  cargo check              --features=gui
guif:
  cargo fmt    --all
  cargo clippy --all --fix --features=gui -- -D warnings

ahkc:
  cargo check              --features=passthru_ahk
ahkf:
  cargo fmt    --all
  cargo clippy --all --fix --features=passthru_ahk -- -D warnings

change_subcrate_versions version:
  sed -i 's/^version = ".*"$/version = "{{version}}"/' parser/Cargo.toml tcp_protocol/Cargo.toml keyberon/Cargo.toml
  sed -i 's/^\(#\? \?kanata-\(keyberon\|parser\|tcp-protocol\).*version\) = "[0-9.]*"/\1 = "{{version}}"/' Cargo.toml parser/Cargo.toml

cov:
  cargo llvm-cov clean --workspace
  cargo llvm-cov --no-report --workspace --no-default-features
  cargo llvm-cov --no-report --workspace
  cargo llvm-cov --no-report --workspace --features=cmd,win_llhook_read_scancodes,win_sendinput_send_scancodes
  cargo llvm-cov --no-report --workspace --features=cmd,interception_driver,win_sendinput_send_scancodes
  cargo llvm-cov --no-report --features=simulated_output -- sim_tests
  cargo llvm-cov report --html

publish:
  cd keyberon && cargo publish
  cd tcp_protocol && cargo publish
  cd parser && cargo publish
  cargo publish

# Include the trailing `\` or `/` in the output_dir parameter. The parameter should be an absolute path.
cfg_to_html output_dir:
  cd docs ; asciidoctor config.adoc
  cd docs ; cp config.html "{{output_dir}}config.html"; rm config.html

# Include the trailing `\` or `/` in the output_dir parameter. The parameter should be an absolute path.
wasm_pack output_dir:
  cd wasm; wasm-pack build --target web; cd pkg; cp kanata_wasm_bg.wasm "{{output_dir}}"; cp kanata_wasm.js "{{output_dir}}"

wasm-build output_dir:
  cd wasm; echo "*" > pkg/.gitignore
  cd wasm; cargo build --lib --release --target wasm32-unknown-unknown
  cd wasm; wasm-bindgen target/wasm32-unknown-unknown/release/kanata_wasm.wasm --out-dir pkg --typescript --target web
  wasm-opt wasm/pkg/kanata_wasm_bg.wasm -o wasm/pkg/kanata_wasm.wasm-opt.wasm -Oz
  rm wasm/pkg/kanata_wasm_bg.wasm
  mv wasm/pkg/kanata_wasm.wasm-opt.wasm wasm/pkg/kanata_wasm_bg.wasm
  cp wasm/pkg/kanata_wasm_bg.wasm "{{output_dir}}"
  cp wasm/pkg/kanata_wasm.js "{{output_dir}}"
