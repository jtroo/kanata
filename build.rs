#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
use indoc::formatdoc;
#[cfg(target_os = "windows")]
use regex::Regex;
#[cfg(target_os = "windows")]
use std::fs::File;
#[cfg(target_os = "windows")]
use std::io::Write;
#[cfg(target_os = "windows")]
extern crate embed_resource;

#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! pb { // println! during build
  ($($tokens:tt)*) => {println!("cargo:warning={}", format!($($tokens)*))}}

#[cfg(target_os = "windows")]
fn main() -> std::io::Result<()> {
    let manifest_path: &str = "./target/kanata.exe.manifest";
    let re_ver_build = Regex::new(r"^(?<vpre>(\d+\.){2}\d+)[-a-zA-Z]+(?<vpos>\d+)$").unwrap();
    let re_version4 = Regex::new(r"^(\d+\.){3}\d+$").unwrap(); // MS says "Use the four-part version format: mmmmm.nnnnn.ooooo.ppppp" https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests
    let re_version3 = Regex::new(r"^(\d+\.){2}\d+$").unwrap();
    let mut version: String = env!("CARGO_PKG_VERSION").to_string();

    if re_version4.find(&version).is_some() {
        // pb!("found 'n.n.n.n' version, leaving as is {}", version);
    } else if re_version3.find(&version).is_some() {
        version = format!("{}.0", version);
        // pb!("found 'n.n.n' version, adding the 4th number {}", version);
    } else if re_ver_build.find(&version).is_some() {
        version = re_ver_build
            .replace_all(&version, r"$vpre.$vpos")
            .to_string();
        // pb!("found 'n.n.n-Word-n' version, removing word '{}'", version);
    } else {
        // pb!("unknown version format '{}', using '0.0.0.0'", version);
        version = "0.0.0.0".to_string();
    }

    let manifest_str = formatdoc!(
        r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?>
        <assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
          <assemblyIdentity name="kanata.exe" version="{}" type="win32"/>
          <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
            <security>
              <requestedPrivileges><requestedExecutionLevel level="asInvoker" uiAccess="false"/></requestedPrivileges>
            </security>
          </trustInfo>
        </assembly>
        "#,
        version
    );
    let mut manifest_f = File::create(manifest_path)?;
    write!(manifest_f, "{}", manifest_str)?;
    embed_resource::compile("./src/kanata.exe.manifest.rc", embed_resource::NONE);
    Ok(())
}
