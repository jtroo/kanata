#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux {
    use super::super::*;

    #[test]
    fn linux_device_parses_properly() {
        let source = r#"
(defcfg linux-device-detect-mode any)
(defsrc) (deflayer base)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("no error");
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::Any)
        );

        let source = r#"
(defcfg linux-device-detect-mode keyboard-only)
(defsrc) (deflayer base)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("no error");
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::KeyboardOnly)
        );

        let source = r#"
(defcfg linux-device-detect-mode keyboard-mice)
(defsrc) (deflayer base)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("no error");
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::KeyboardMice)
        );

        let source = r#"(defsrc mmid) (deflayer base 1)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("no error");
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::Any)
        );

        let source = r#"(defsrc a) (deflayer base b)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("no error");
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::KeyboardMice)
        );

        let source = r#"
(defcfg linux-device-detect-mode not an opt)
(defsrc) (deflayer base)"#;
        parse_cfg(source)
            .map(|_| ())
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect_err("error should happen");
    }

    #[test]
    fn deflayermap_mouse_sets_detect_mode_any() {
        // Issue #2096: a mouse button listed ONLY in deflayermap (not defsrc) must still
        // cause Linux to grab mouse devices (DeviceDetectMode::Any), because parse_layers
        // inserts the mouse OsCode into mapped_keys.
        let source = r#"
(defsrc)
(deflayermap (base)
  mmid v
)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("parses");
        assert!(icfg.mapped_keys.contains(&OsCode::BTN_MIDDLE));
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::Any)
        );
    }

    #[test]
    fn deflayermap_without_mouse_stays_keyboard_mice() {
        // Negative control: a keyboard-only deflayermap must NOT flip the mode to Any.
        let source = r#"
(defsrc a)
(deflayermap (base)
  b c
)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("parses");
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::KeyboardMice)
        );
    }

    #[test]
    fn deflayermap_mouse_with_explicit_defcfg_mode_keeps_override() {
        // An explicit `linux-device-detect-mode` in defcfg must win even when a mouse button is
        // also named in deflayermap: the is_none() guard skips auto-derivation, so the user's
        // choice is preserved (mmid still lands in mapped_keys, but it must not flip the mode).
        let source = r#"
(defcfg linux-device-detect-mode keyboard-only)
(defsrc)
(deflayermap (base)
  mmid v
)"#;
        let icfg = parse_cfg(source)
            .map_err(|e| log::info!("{:?}", miette::Error::from(e)))
            .expect("parses");
        assert!(icfg.mapped_keys.contains(&OsCode::BTN_MIDDLE));
        assert_eq!(
            icfg.options.linux_opts.linux_device_detect_mode,
            Some(DeviceDetectMode::KeyboardOnly)
        );
    }
}
