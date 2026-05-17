// const LP: &str = "cmd-out:";

// #[cfg(not(feature = "simulated_output"))]
pub(super) fn cmd_run_and_check_status(cmd_and_args: &[&str]) -> i32 {
    let mut args = cmd_and_args.iter();
    let mut cmd = std::process::Command::new(
        args.next()
            .expect("parsing should have forbidden empty cmd"),
    );
    for arg in args {
        cmd.arg(arg);
    }
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_e) => {
            // log::error!("Failed to execute cmd: {e}");
            return -1;
        }
    };
    // log::debug!("{LP} stdout: {}", String::from_utf8_lossy(&output.stderr));
    // log::debug!("{LP} stderr: {}", String::from_utf8_lossy(&output.stderr));
    output.status.code().unwrap_or(-1)
}

// #[cfg(feature = "simulated_output")]
// pub(super) fn keys_for_cmd_output(cmd_and_args: &[&str]) -> impl Iterator<Item = Item> {
//     println!("cmd-keys:{cmd_and_args:?}");
//     [].iter().copied()
// }
