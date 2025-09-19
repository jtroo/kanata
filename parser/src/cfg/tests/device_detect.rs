#[cfg(target_os = "linux")]
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
}
