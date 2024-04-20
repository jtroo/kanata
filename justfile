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
  cargo build --release --features win_manifest; cp target/release/kanata.exe "{{output_dir}}\kanata.exe"
  cargo build --release --features win_manifest,interception_driver; cp target/release/kanata.exe "{{output_dir}}\kanata_wintercept.exe"
  cargo build --release --features win_manifest,win_sendinput_send_scancodes; cp target/release/kanata.exe "{{output_dir}}\kanata_scancode_experimental.exe"
  cargo build --release --features win_manifest,win_sendinput_send_scancodes,win_llhook_read_scancodes; cp target/release/kanata.exe "{{output_dir}}\kanata_winIOv2.exe"
  cargo build --release --features win_manifest,cmd; cp target/release/kanata.exe "{{output_dir}}\kanata_cmd_allowed.exe"
  cargo build --release --features win_manifest,cmd,interception_driver; cp target/release/kanata.exe "{{output_dir}}\kanata_wintercept_cmd_allowed.exe"
  cp cfg_samples/kanata.kbd "{{output_dir}}"

# Generate the sha256sums for all files in the output directory
sha256sums output_dir:
  rm -f {{output_dir}}/sha256sums
  cd {{output_dir}}; sha256sum * > sha256sums

test:
  cargo test --verbose -p kanata -p kanata-parser -p kanata-keyberon -- --nocapture
  cargo clippy --all

fmt:
  cargo fmt --all

use_cratesio_deps:
  sed -i 's/^# \(kanata-\(keyberon\|parser\|tcp-protocol\) = ".*\)$/\1/' Cargo.toml parser/Cargo.toml
  sed -i 's/^\(kanata-\(keyberon\|parser\|tcp-protocol\) = .*path.*\)$/# \1/' Cargo.toml parser/Cargo.toml

use_local_deps:
  sed -i 's/^\(kanata-\(keyberon\|parser\|tcp-protocol\) = ".*\)$/# \1/' Cargo.toml parser/Cargo.toml
  sed -i 's/^# \(kanata-\(keyberon\|parser\|tcp-protocol\) = .*path.*\)$/\1/' Cargo.toml parser/Cargo.toml

change_subcrate_versions version:
  sed -i 's/^version = ".*"$/version = "{{version}}"/' parser/Cargo.toml tcp_protocol/Cargo.toml keyberon/Cargo.toml
  sed -i 's/^\(#\? \?kanata-\(keyberon\|parser\|tcp-protocol\)\) = ".*$/\1 = "{{version}}"/' Cargo.toml parser/Cargo.toml
