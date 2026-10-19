//! `kanata msg`: a command-line client for the TCP server of a running kanata.
//!
//! Each invocation sends one message, waits for the reply that belongs to it
//! while skipping unrelated notifications, and fails if kanata reported an
//! error.

use anyhow::{Context, Result, bail};
use kanata_state_machine::SocketAddrWrapper;
use kanata_tcp_protocol::{ClientMessage, FakeKeyActionMessage, ServerMessage, ServerResponse};
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

const TIMED_OUT: &str = "timed out waiting for a reply from kanata";

#[derive(clap::Args, Debug)]
pub struct MsgArgs {
    /// Port or full address (IP:PORT) of the kanata TCP server.
    #[arg(short, long, value_name = "PORT or IP:PORT")]
    port: SocketAddrWrapper,

    /// Milliseconds to wait for a reply from kanata.
    #[arg(long, default_value_t = 5000)]
    #[arg(value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,

    /// Print the replies from kanata as raw JSON lines.
    #[arg(long)]
    json: bool,

    #[command(subcommand)]
    cmd: MsgCommand,
}

#[derive(clap::Subcommand, Debug)]
enum MsgCommand {
    /// Change the base layer. Fails if the layer does not exist.
    ChangeLayer { name: String },
    /// Print the names of all layers, one per line.
    ListLayers,
    /// Print the name of the active layer.
    CurrentLayer,
    /// Print the configuration of the active layer.
    CurrentLayerInfo,
    /// Print the names of all virtual keys, one per line.
    ListFakeKeys,
    /// Act on a virtual key. Fails if the key does not exist.
    ActOnFakeKey {
        name: String,
        #[arg(value_enum, default_value_t = FakeKeyAction::Tap)]
        action: FakeKeyAction,
    },
    /// Move the mouse cursor. kanata does not report whether this succeeded.
    SetMouse { x: u16, y: u16 },
    /// Reload the current configuration file.
    Reload(ReloadOpts),
    /// Reload the next configuration file.
    ReloadNext(ReloadOpts),
    /// Reload the previous configuration file.
    ReloadPrev(ReloadOpts),
    /// Reload the configuration file at a zero-based index of the --cfg list.
    ReloadNum {
        index: usize,
        #[command(flatten)]
        opts: ReloadOpts,
    },
    /// Reload a configuration file. Relative paths are resolved by kanata.
    ReloadFile {
        path: String,
        #[command(flatten)]
        opts: ReloadOpts,
    },
    /// Print the kanata version, protocol version and capabilities.
    Hello,
    /// Print every message from kanata as a JSON line until the connection closes.
    EventStream,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum FakeKeyAction {
    Press,
    Release,
    Tap,
    Toggle,
}

impl From<FakeKeyAction> for FakeKeyActionMessage {
    fn from(action: FakeKeyAction) -> Self {
        match action {
            FakeKeyAction::Press => Self::Press,
            FakeKeyAction::Release => Self::Release,
            FakeKeyAction::Tap => Self::Tap,
            FakeKeyAction::Toggle => Self::Toggle,
        }
    }
}

#[derive(clap::Args, Debug)]
struct ReloadOpts {
    /// Wait for the reload to finish and fail if it did not succeed.
    /// Without this, success only means that kanata accepted the request.
    #[arg(long)]
    wait: bool,
}

impl ReloadOpts {
    /// The `wait` and `timeout_ms` fields of a reload message.
    fn fields(&self, timeout: Duration) -> (Option<bool>, Option<u64>) {
        if self.wait {
            (Some(true), Some(timeout.as_millis() as u64))
        } else {
            (None, None)
        }
    }
}

pub fn run(args: MsgArgs) -> Result<()> {
    let timeout = Duration::from_millis(args.timeout_ms);
    let mut conn = Conn::connect(args.port.get_ref(), timeout)?;
    let mut stdout = std::io::stdout().lock();
    execute(args.cmd, &mut conn, args.json, &mut stdout)
}

fn execute(cmd: MsgCommand, conn: &mut Conn, json: bool, out: &mut impl Write) -> Result<()> {
    match cmd {
        MsgCommand::ChangeLayer { name } => {
            // kanata silently ignores unknown layers, so check the name first.
            conn.send(&ClientMessage::RequestLayerNames {})?;
            let (_, names) = conn.wait_for(layer_names)?;
            if !names.contains(&name) {
                let names = names.join(", ");
                bail!("unknown layer: {name}\navailable layers: {names}");
            }
            conn.send(&ClientMessage::ChangeLayer { new: name })?;
            conn.sync()?;
        }
        MsgCommand::ListLayers => {
            conn.send(&ClientMessage::RequestLayerNames {})?;
            let (line, names) = conn.wait_for(layer_names)?;
            output(out, json, &line, &names)?;
        }
        MsgCommand::CurrentLayer => {
            conn.send(&ClientMessage::RequestCurrentLayerName {})?;
            let (line, name) = conn.wait_for(|r| match r {
                Reply::Message(ServerMessage::CurrentLayerName { name }) => Some(name),
                _ => None,
            })?;
            output(out, json, &line, &[name])?;
        }
        MsgCommand::CurrentLayerInfo => {
            conn.send(&ClientMessage::RequestCurrentLayerInfo {})?;
            let (line, cfg_text) = conn.wait_for(|r| match r {
                Reply::Message(ServerMessage::CurrentLayerInfo { cfg_text, .. }) => Some(cfg_text),
                _ => None,
            })?;
            output(out, json, &line, &[cfg_text])?;
        }
        MsgCommand::ListFakeKeys => {
            conn.send(&ClientMessage::RequestFakeKeyNames {})?;
            let (line, names) = conn.wait_for(|r| match r {
                Reply::Message(ServerMessage::FakeKeyNames { names }) => Some(names),
                _ => None,
            })?;
            output(out, json, &line, &names)?;
        }
        MsgCommand::ActOnFakeKey { name, action } => {
            conn.send(&ClientMessage::ActOnFakeKey {
                name,
                action: action.into(),
            })?;
            conn.sync()?;
        }
        MsgCommand::SetMouse { x, y } => {
            conn.send(&ClientMessage::SetMouse { x, y })?;
            conn.sync()?;
        }
        MsgCommand::Reload(opts) => {
            let (wait, timeout_ms) = opts.fields(conn.timeout);
            let msg = ClientMessage::Reload { wait, timeout_ms };
            reload(conn, msg, opts.wait, json, out)?;
        }
        MsgCommand::ReloadNext(opts) => {
            let (wait, timeout_ms) = opts.fields(conn.timeout);
            let msg = ClientMessage::ReloadNext { wait, timeout_ms };
            reload(conn, msg, opts.wait, json, out)?;
        }
        MsgCommand::ReloadPrev(opts) => {
            let (wait, timeout_ms) = opts.fields(conn.timeout);
            let msg = ClientMessage::ReloadPrev { wait, timeout_ms };
            reload(conn, msg, opts.wait, json, out)?;
        }
        MsgCommand::ReloadNum { index, opts } => {
            let (wait, timeout_ms) = opts.fields(conn.timeout);
            let msg = ClientMessage::ReloadNum {
                index,
                wait,
                timeout_ms,
            };
            reload(conn, msg, opts.wait, json, out)?;
        }
        MsgCommand::ReloadFile { path, opts } => {
            let (wait, timeout_ms) = opts.fields(conn.timeout);
            let msg = ClientMessage::ReloadFile {
                path,
                wait,
                timeout_ms,
            };
            reload(conn, msg, opts.wait, json, out)?;
        }
        MsgCommand::Hello => {
            conn.send(&ClientMessage::Hello {})?;
            let (line, text) = conn.wait_for(|r| match r {
                Reply::Message(ServerMessage::HelloOk {
                    version,
                    protocol,
                    capabilities,
                }) => Some([
                    format!("version: {version}"),
                    format!("protocol: {protocol}"),
                    format!("capabilities: {}", capabilities.join(" ")),
                ]),
                _ => None,
            })?;
            output(out, json, &line, &text)?;
        }
        MsgCommand::EventStream => {
            conn.deadline = None;
            loop {
                let (line, _) = conn.recv()?;
                writeln!(out, "{line}")?;
            }
        }
    }
    Ok(())
}

/// Send a reload message; with `wait`, also wait for the result of the reload.
fn reload(
    conn: &mut Conn,
    msg: ClientMessage,
    wait: bool,
    json: bool,
    out: &mut impl Write,
) -> Result<()> {
    conn.send(&msg)?;
    let (line, ()) = conn.wait_for(|r| match r {
        Reply::Response(ServerResponse::Ok) => Some(()),
        _ => None,
    })?;
    output(out, json, &line, &[])?;
    if !wait {
        return Ok(());
    }
    // kanata starts its own timeout only after the reply above, so allow some
    // slack for the result to arrive.
    conn.restart_timeout(Duration::from_secs(1));
    let (line, (ok, timeout_ms)) = conn.wait_for(|r| match r {
        Reply::Message(ServerMessage::ReloadResult { ok, timeout_ms }) => Some((ok, timeout_ms)),
        _ => None,
    })?;
    output(out, json, &line, &[])?;
    if let Some(ms) = timeout_ms {
        bail!("reload did not finish within {ms} ms");
    }
    if !ok {
        bail!("reload failed; check the kanata log for details");
    }
    Ok(())
}

fn layer_names(reply: Reply) -> Option<Vec<String>> {
    match reply {
        Reply::Message(ServerMessage::LayerNames { names }) => Some(names),
        _ => None,
    }
}

/// Print the raw reply `line` for `--json`, otherwise `text` line by line.
fn output(out: &mut impl Write, json: bool, line: &str, text: &[String]) -> Result<()> {
    if json {
        writeln!(out, "{line}")?;
    } else {
        for t in text {
            writeln!(out, "{t}")?;
        }
    }
    Ok(())
}

/// A line from kanata that this client understands.
enum Reply {
    Response(ServerResponse),
    Message(ServerMessage),
}

struct Conn {
    stream: BufReader<TcpStream>,
    timeout: Duration,
    /// When to stop waiting for a reply; `None` waits forever.
    deadline: Option<Instant>,
}

impl Conn {
    fn connect(addr: &SocketAddr, timeout: Duration) -> Result<Self> {
        let stream = TcpStream::connect_timeout(addr, timeout)
            .with_context(|| format!("could not connect to kanata at {addr}"))?;
        Ok(Self {
            stream: BufReader::new(stream),
            timeout,
            deadline: Instant::now().checked_add(timeout),
        })
    }

    /// Restart the reply timeout, allowing `extra` time on top of it.
    fn restart_timeout(&mut self, extra: Duration) {
        let timeout = self.timeout.saturating_add(extra);
        self.deadline = Instant::now().checked_add(timeout);
    }

    fn send(&mut self, msg: &ClientMessage) -> Result<()> {
        let mut line = serde_json::to_vec(msg)?;
        line.push(b'\n');
        self.stream.get_mut().write_all(&line)?;
        Ok(())
    }

    /// Read the next line, along with its parsed form if this client
    /// understands it, so that new message types from a newer kanata are
    /// skipped instead of being fatal.
    fn recv(&mut self) -> Result<(String, Option<Reply>)> {
        let now = Instant::now();
        let timeout = self.deadline.map(|d| d.saturating_duration_since(now));
        if timeout == Some(Duration::ZERO) {
            bail!(TIMED_OUT);
        }
        self.stream.get_ref().set_read_timeout(timeout)?;
        let mut line = String::new();
        match self.stream.read_line(&mut line) {
            Ok(0) => bail!("kanata closed the connection"),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                bail!(TIMED_OUT)
            }
            Err(e) => return Err(e.into()),
        }
        let reply = serde_json::from_str(&line)
            .map(Reply::Response)
            .or_else(|_| serde_json::from_str(&line).map(Reply::Message))
            .ok();
        Ok((line.trim_end().to_owned(), reply))
    }

    /// Read lines until `select` accepts one, skipping notifications that are
    /// unrelated to the request. An error reported by kanata fails the request.
    fn wait_for<T>(&mut self, mut select: impl FnMut(Reply) -> Option<T>) -> Result<(String, T)> {
        loop {
            let (line, reply) = self.recv()?;
            match reply {
                Some(Reply::Response(ServerResponse::Error { msg }))
                | Some(Reply::Message(ServerMessage::Error { msg })) => bail!("kanata: {msg}"),
                Some(reply) => {
                    if let Some(v) = select(reply) {
                        return Ok((line, v));
                    }
                }
                None => {}
            }
        }
    }

    /// Wait until kanata has processed every message sent so far.
    ///
    /// kanata handles the messages of a connection in order and never
    /// broadcasts `CurrentLayerName`, so this reply arrives after any error
    /// that an earlier message caused.
    fn sync(&mut self) -> Result<()> {
        self.send(&ClientMessage::RequestCurrentLayerName {})?;
        self.wait_for(|r| match r {
            Reply::Message(ServerMessage::CurrentLayerName { .. }) => Some(()),
            _ => None,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    const LAYER_NAMES: &str = r#"{"LayerNames":{"names":["base","nav"]}}"#;
    const CURRENT_LAYER: &str = r#"{"CurrentLayerName":{"name":"base"}}"#;
    const TAP: &str = r#"{"TapActivated":{"key":"a"}}"#;
    const FAKE_KEY_ERROR: &str = r#"{"Error":{"msg":"unknown virtual/fake key: x"}}"#;
    const RESPONSE_OK: &str = r#"{"status":"Ok"}"#;
    const RELOAD_FAILED: &str = r#"{"ReloadResult":{"ok":false}}"#;

    /// Accept one client and behave like kanata: send the initial layer, then
    /// answer each message with the lines that `respond` returns.
    fn fake_kanata(
        respond: impl Fn(ClientMessage) -> Vec<&'static str> + Send + 'static,
    ) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.write_all(b"{\"LayerChange\":{\"new\":\"base\"}}\n");
            let reader = BufReader::new(stream.try_clone().unwrap());
            for line in reader.lines().map_while(|line| line.ok()) {
                for reply in respond(line.parse().unwrap()) {
                    let _ = stream.write_all(format!("{reply}\n").as_bytes());
                }
            }
        });
        addr
    }

    fn exec(addr: SocketAddr, cmd: MsgCommand, json: bool) -> Result<String> {
        let mut conn = Conn::connect(&addr, Duration::from_secs(5))?;
        let mut out: Vec<u8> = Vec::new();
        execute(cmd, &mut conn, json, &mut out)?;
        Ok(String::from_utf8(out)?)
    }

    #[test]
    fn list_layers_skips_notifications() {
        let addr = fake_kanata(|msg| match msg {
            ClientMessage::RequestLayerNames {} => vec![TAP, LAYER_NAMES],
            _ => vec![],
        });
        let out = exec(addr, MsgCommand::ListLayers, false).unwrap();
        assert_eq!(out, "base\nnav\n");
    }

    #[test]
    fn json_prints_raw_reply() {
        let addr = fake_kanata(|msg| match msg {
            ClientMessage::RequestCurrentLayerName {} => vec![CURRENT_LAYER],
            _ => vec![],
        });
        let out = exec(addr, MsgCommand::CurrentLayer, true).unwrap();
        assert_eq!(out, format!("{CURRENT_LAYER}\n"));
    }

    #[test]
    fn change_layer_rejects_unknown_layer() {
        let addr = fake_kanata(|msg| match msg {
            ClientMessage::RequestLayerNames {} => vec![LAYER_NAMES],
            _ => vec![],
        });
        let cmd = MsgCommand::ChangeLayer { name: "num".into() };
        let err = exec(addr, cmd, false).unwrap_err();
        assert!(err.to_string().contains("available layers: base, nav"));
    }

    #[test]
    fn act_on_fake_key_reports_error() {
        let addr = fake_kanata(|msg| match msg {
            ClientMessage::ActOnFakeKey { .. } => vec![FAKE_KEY_ERROR],
            ClientMessage::RequestCurrentLayerName {} => vec![CURRENT_LAYER],
            _ => vec![],
        });
        let cmd = MsgCommand::ActOnFakeKey {
            name: "x".into(),
            action: FakeKeyAction::Tap,
        };
        let err = exec(addr, cmd, false).unwrap_err();
        assert!(err.to_string().contains("unknown virtual/fake key"));
    }

    #[test]
    fn reload_wait_reports_failure() {
        let addr = fake_kanata(|msg| match msg {
            ClientMessage::Reload { .. } => vec![RESPONSE_OK, RELOAD_FAILED],
            _ => vec![],
        });
        let cmd = MsgCommand::Reload(ReloadOpts { wait: true });
        assert!(exec(addr, cmd, false).is_err());
    }

    #[test]
    fn missing_reply_times_out() {
        let addr = fake_kanata(|_| vec![]);
        let mut conn = Conn::connect(&addr, Duration::from_millis(200)).unwrap();
        let mut out: Vec<u8> = Vec::new();
        let err = execute(MsgCommand::CurrentLayer, &mut conn, false, &mut out).unwrap_err();
        assert!(err.to_string().contains("timed out"));
    }
}
