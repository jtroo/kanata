fn main() -> std::io::Result<()> {
    #[cfg(all(target_os = "windows", feature = "win_manifest"))]
    {
        windows::build()?;
    }
    Ok(())
}

#[cfg(all(target_os = "windows", feature = "win_manifest"))]
mod windows {
    use indoc::formatdoc;
    use regex::Regex;
    use std::fs::File;
    use std::io::Write;
    extern crate embed_resource;

    // println! during build
    macro_rules! pb {
      ($($tokens:tt)*) => {println!("cargo:warning={}", format!($($tokens)*))}}

    pub(super) fn build() -> std::io::Result<()> {
        let manifest_path: &str = "./target/kanata.exe.manifest";

        // Note about expected version format:
        // MS says "Use the four-part version format: mmmmm.nnnnn.ooooo.ppppp"
        // https://learn.microsoft.com/en-us/windows/win32/sbscs/application-manifests

        let re_ver_build = Regex::new(r"^(?<vpre>(\d+\.){2}\d+)[-a-zA-Z]+(?<vpos>\d+)$").unwrap();
        let re_version3 = Regex::new(r"^(\d+\.){2}\d+$").unwrap();
        let mut version: String = env!("CARGO_PKG_VERSION").to_string();

        if re_version3.find(&version).is_some() {
            version = format!("{}.0", version);
        } else if re_ver_build.find(&version).is_some() {
            version = re_ver_build
                .replace_all(&version, r"$vpre.$vpos")
                .to_string();
        } else {
            pb!("unknown version format '{}', using '0.0.0.0'", version);
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
}
