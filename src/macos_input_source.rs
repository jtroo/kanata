use anyhow::{Context, Result, anyhow, bail};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::ptr;
use std::thread::sleep;
use std::time::Duration;

type CFArrayRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFMutableDictionaryRef = *mut c_void;
type CFTypeRef = *const c_void;
type TISInputSourceRef = *const c_void;

const NO_ERR: i32 = 0;
const INPUT_SOURCE_VERIFY_DELAY_MS: u64 = 50;
const SOCKET_PREFIX: &str = "kanata-input-source-helper";

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;

    fn CFArrayGetCount(theArray: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(theArray: CFArrayRef, idx: isize) -> *const c_void;
    fn CFDictionaryCreateMutable(
        allocator: *const c_void,
        capacity: isize,
        keyCallBacks: *const c_void,
        valueCallBacks: *const c_void,
    ) -> CFMutableDictionaryRef;
    fn CFDictionarySetValue(
        theDict: CFMutableDictionaryRef,
        key: *const c_void,
        value: *const c_void,
    );
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    static kTISPropertyInputSourceID: CFStringRef;
    static kTISPropertyInputModeID: CFStringRef;
    static kTISPropertyLocalizedName: CFStringRef;
    static kTISPropertyInputSourceCategory: CFStringRef;
    static kTISPropertyInputSourceIsEnabled: CFStringRef;
    static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
    static kTISCategoryKeyboardInputSource: CFStringRef;

    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISCreateInputSourceList(
        properties: *const c_void,
        includeAllInstalledInputSources: u8,
    ) -> CFArrayRef;
    fn TISGetInputSourceProperty(
        inputSource: TISInputSourceRef,
        propertyKey: CFStringRef,
    ) -> *const c_void;
    fn TISSelectInputSource(inputSource: TISInputSourceRef) -> i32;
}

pub fn set_current_input_source_by_id_via_helper(id: &str) -> Result<()> {
    match send_helper_request(&format!("set\t{}", escape_field(id)))? {
        HelperResponse::Ok(None) => Ok(()),
        HelperResponse::Ok(Some(_)) => Ok(()),
        HelperResponse::Err(error) => Err(anyhow!(error)),
    }
}

pub fn current_input_source_is_via_helper(id: &str) -> Result<bool> {
    match send_helper_request("current")? {
        HelperResponse::Ok(Some(current_id)) => Ok(current_id == id),
        HelperResponse::Ok(None) => Ok(false),
        HelperResponse::Err(error) => Err(anyhow!(error)),
    }
}

pub fn serve_helper_forever() -> Result<()> {
    let socket_path = helper_socket_path_for_current_user();
    prepare_socket_path(&socket_path)?;
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind {}", socket_path.display()))?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {}", socket_path.display()))?;
    let _cleanup = SocketCleanup {
        path: socket_path.clone(),
    };

    log::info!(
        "macOS input-source helper listening at {}",
        socket_path.display()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(e) = handle_helper_stream(stream) {
                    log::error!("failed to handle macOS input-source helper request: {e}");
                }
            }
            Err(e) => log::error!("failed to accept macOS input-source helper connection: {e}"),
        }
    }

    Ok(())
}

fn send_helper_request(request: &str) -> Result<HelperResponse> {
    let uid = console_user_uid();
    let socket_path = helper_socket_path_for_uid(uid);
    validate_helper_socket(&socket_path, uid)?;
    let mut stream = UnixStream::connect(&socket_path).with_context(|| {
        format!(
            "macOS input-source helper is not running at {}. Start kanata-input-source-helper as the logged-in console user.",
            socket_path.display()
        )
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set macOS input-source helper read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .context("failed to set macOS input-source helper write timeout")?;

    stream
        .write_all(request.as_bytes())
        .context("failed to send request to macOS input-source helper")?;
    stream
        .write_all(b"\n")
        .context("failed to finish request to macOS input-source helper")?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .context("failed to close macOS input-source helper request stream")?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .context("failed to read response from macOS input-source helper")?;
    parse_helper_response(response.trim_end())
}

fn handle_helper_stream(mut stream: UnixStream) -> Result<()> {
    let mut request = String::new();
    BufReader::new(stream.try_clone()?)
        .read_line(&mut request)
        .context("failed to read macOS input-source helper request")?;

    let response = match handle_helper_request(request.trim_end()) {
        Ok(response) => response,
        Err(e) => format!("err\t{}", escape_field(&e.to_string())),
    };

    stream
        .write_all(response.as_bytes())
        .context("failed to write macOS input-source helper response")?;
    stream
        .write_all(b"\n")
        .context("failed to finish macOS input-source helper response")?;
    Ok(())
}

fn handle_helper_request(request: &str) -> Result<String> {
    let mut parts = request.splitn(2, '\t');
    match parts.next() {
        Some("current") => {
            let current_id = current_keyboard_input_source_id()?;
            Ok(format!(
                "ok\t{}",
                escape_field(current_id.as_deref().unwrap_or_default())
            ))
        }
        Some("set") => {
            let Some(id) = parts.next() else {
                bail!("missing input source ID in set request");
            };
            let id = unescape_field(id)?;
            set_current_input_source_by_id(&id).map(|()| "ok".to_owned())
        }
        Some(op) if !op.is_empty() => bail!("unknown macOS input-source helper request: {op:?}"),
        _ => bail!("empty macOS input-source helper request"),
    }
}

fn parse_helper_response(response: &str) -> Result<HelperResponse> {
    let mut parts = response.splitn(2, '\t');
    match parts.next() {
        Some("ok") => Ok(HelperResponse::Ok(match parts.next() {
            Some(payload) => Some(unescape_field(payload)?),
            None => None,
        })),
        Some("err") => Ok(HelperResponse::Err(match parts.next() {
            Some(payload) => unescape_field(payload)?,
            None => "macOS input-source helper returned an unspecified error".to_owned(),
        })),
        Some(status) if !status.is_empty() => {
            bail!("invalid macOS input-source helper response status: {status:?}")
        }
        _ => bail!("empty response from macOS input-source helper"),
    }
}

fn prepare_socket_path(socket_path: &Path) -> Result<()> {
    let uid = unsafe { libc::geteuid() };
    let socket_dir = helper_socket_dir_for_uid(uid);
    prepare_socket_dir(&socket_dir, uid)?;

    if UnixStream::connect(socket_path).is_ok() {
        bail!(
            "macOS input-source helper is already running at {}",
            socket_path.display()
        );
    }

    match fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| {
            format!(
                "failed to remove stale macOS input-source helper socket {}",
                socket_path.display()
            )
        }),
    }
}

fn helper_socket_path_for_current_user() -> PathBuf {
    helper_socket_path_for_uid(unsafe { libc::geteuid() })
}

fn helper_socket_dir_for_uid(uid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/{SOCKET_PREFIX}-{uid}"))
}

fn helper_socket_path_for_uid(uid: u32) -> PathBuf {
    helper_socket_dir_for_uid(uid).join("socket")
}

fn prepare_socket_dir(socket_dir: &Path, uid: u32) -> Result<()> {
    match fs::symlink_metadata(socket_dir) {
        Ok(_) => validate_socket_dir(socket_dir, uid)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(socket_dir)
                .with_context(|| format!("failed to create {}", socket_dir.display()))?;
        }
        Err(e) => {
            return Err(e).with_context(|| format!("failed to inspect {}", socket_dir.display()));
        }
    }

    fs::set_permissions(socket_dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to set permissions on {}", socket_dir.display()))?;
    validate_socket_dir(socket_dir, uid)
}

fn validate_helper_socket(socket_path: &Path, uid: u32) -> Result<()> {
    let socket_dir = socket_path.parent().ok_or_else(|| {
        anyhow!(
            "helper socket path has no parent: {}",
            socket_path.display()
        )
    })?;

    match fs::symlink_metadata(socket_dir) {
        Ok(_) => validate_socket_dir(socket_dir, uid)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(e).with_context(|| format!("failed to inspect {}", socket_dir.display()));
        }
    }

    let metadata = match fs::symlink_metadata(socket_path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(e).with_context(|| format!("failed to inspect {}", socket_path.display()));
        }
    };
    let file_type = metadata.file_type();
    if !file_type.is_socket() {
        bail!(
            "macOS input-source helper path is not a Unix socket: {}",
            socket_path.display()
        );
    }
    if metadata.uid() != uid {
        bail!(
            "macOS input-source helper socket {} is owned by uid {}, expected uid {}",
            socket_path.display(),
            metadata.uid(),
            uid
        );
    }
    if metadata.mode() & 0o077 != 0 {
        bail!(
            "macOS input-source helper socket {} has overly broad permissions {:o}",
            socket_path.display(),
            metadata.mode() & 0o777
        );
    }
    Ok(())
}

fn validate_socket_dir(socket_dir: &Path, uid: u32) -> Result<()> {
    let metadata = fs::symlink_metadata(socket_dir)
        .with_context(|| format!("failed to inspect {}", socket_dir.display()))?;
    let file_type = metadata.file_type();
    if !file_type.is_dir() {
        bail!(
            "macOS input-source helper socket directory is not a directory: {}",
            socket_dir.display()
        );
    }
    if metadata.uid() != uid {
        bail!(
            "macOS input-source helper socket directory {} is owned by uid {}, expected uid {}",
            socket_dir.display(),
            metadata.uid(),
            uid
        );
    }
    if metadata.mode() & 0o077 != 0 {
        bail!(
            "macOS input-source helper socket directory {} has overly broad permissions {:o}",
            socket_dir.display(),
            metadata.mode() & 0o777
        );
    }
    Ok(())
}

fn console_user_uid() -> u32 {
    fs::metadata("/dev/console")
        .map(|metadata| metadata.uid())
        .unwrap_or_else(|e| {
            let uid = unsafe { libc::geteuid() };
            log::warn!(
                "failed to read /dev/console owner for macOS input-source helper lookup: {e}; using effective UID {uid}"
            );
            uid
        })
}

fn set_current_input_source_by_id(id: &str) -> Result<()> {
    with_input_source_list(|list, count| {
        let before_select_id = current_keyboard_input_source_id();

        let sources = collect_selectable_keyboard_sources(list, count);

        let matching_sources = sources
            .into_iter()
            .filter(|(_, info)| info.id.as_deref() == Some(id))
            .collect::<Vec<_>>();

        let Some((source, info)) = matching_sources
            .iter()
            .find(|(_, info)| info.is_enabled && info.is_select_capable)
        else {
            let diagnostics = input_source_diagnostics(id, &before_select_id, &matching_sources);
            if matching_sources.is_empty() {
                return Err(anyhow!(
                    "macOS input source ID not found in selectable enabled sources: {id:?}. {diagnostics}"
                ));
            }
            return Err(anyhow!(
                "macOS input source ID {id:?} was found, but no matching source is both enabled and select-capable. Matches: {}. {diagnostics}",
                format_matches(&matching_sources),
            ));
        };

        let status = unsafe { TISSelectInputSource(*source) };

        let immediate_id = current_keyboard_input_source_id();
        sleep(Duration::from_millis(INPUT_SOURCE_VERIFY_DELAY_MS));
        let delayed_id = current_keyboard_input_source_id();

        if status != NO_ERR {
            let diagnostics = input_source_diagnostics(id, &before_select_id, &matching_sources);
            return Err(anyhow!(
                "failed to select macOS input source {id:?}; TISSelectInputSource returned OSStatus {status}. Selected candidate: {info}. Current input source before selection: {}; immediately after: {}; after {INPUT_SOURCE_VERIFY_DELAY_MS}ms: {}. {diagnostics}",
                format_current_read_result(&before_select_id),
                format_current_read_result(&immediate_id),
                format_current_read_result(&delayed_id)
            ));
        }

        if current_id_matches(&immediate_id, id) || current_id_matches(&delayed_id, id) {
            return Ok(());
        }

        let diagnostics = input_source_diagnostics(id, &before_select_id, &matching_sources);
        Err(anyhow!(
            "TISSelectInputSource returned success for macOS input source {id:?}, but the current keyboard input source did not change to the requested ID. Selected candidate: {info}. Current input source before selection: {}; immediately after: {}; after {INPUT_SOURCE_VERIFY_DELAY_MS}ms: {}. {diagnostics}",
            format_current_read_result(&before_select_id),
            format_current_read_result(&immediate_id),
            format_current_read_result(&delayed_id)
        ))
    })
}

fn collect_selectable_keyboard_sources(
    list: CFArrayRef,
    count: isize,
) -> Vec<(TISInputSourceRef, InputSourceInfo)> {
    let mut sources = Vec::new();

    for idx in 0..count {
        let source = input_source_at(list, idx);
        if source.is_null() {
            continue;
        }
        let info = InputSourceInfo::new(source);
        sources.push((source, info));
    }

    sources
}

fn input_source_diagnostics(
    requested_id: &str,
    current_id: &Result<Option<String>>,
    selectable_sources: &[(TISInputSourceRef, InputSourceInfo)],
) -> String {
    let list = unsafe { TISCreateInputSourceList(ptr::null(), 1) };
    if list.is_null() {
        return "failed to list all installed macOS input sources for diagnostics".to_owned();
    }

    let count = unsafe { CFArrayGetCount(list) };
    let mut installed_sources = Vec::new();
    for idx in 0..count {
        let source = input_source_at(list, idx);
        if source.is_null() {
            continue;
        }
        let info = InputSourceInfo::new(source);
        installed_sources.push(info);
    }
    unsafe { CFRelease(list as CFTypeRef) };

    let selectable_id_matches = selectable_sources
        .iter()
        .filter(|(_, info)| info.id.as_deref() == Some(requested_id))
        .map(|(_, info)| info)
        .collect::<Vec<_>>();
    let installed_id_matches = installed_sources
        .iter()
        .filter(|info| info.id.as_deref() == Some(requested_id))
        .collect::<Vec<_>>();
    let installed_mode_matches = installed_sources
        .iter()
        .filter(|info| info.input_mode_id.as_deref() == Some(requested_id))
        .collect::<Vec<_>>();
    format!(
        "Diagnostics: current={}, selectable input_source_id matches={}, installed input_source_id matches={}, installed input_mode_id matches={}",
        format_current_read_result(current_id),
        format_info_refs(&selectable_id_matches),
        format_info_refs(&installed_id_matches),
        format_info_refs(&installed_mode_matches),
    )
}

fn current_id_matches(current_id: &Result<Option<String>>, id: &str) -> bool {
    matches!(current_id, Ok(Some(current_id)) if current_id == id)
}

fn format_current_read_result(current_id: &Result<Option<String>>) -> String {
    match current_id {
        Ok(id) => format!("{id:?}"),
        Err(e) => format!("read error: {e}"),
    }
}

fn current_keyboard_input_source_id() -> Result<Option<String>> {
    let source = unsafe { TISCopyCurrentKeyboardInputSource() };
    if source.is_null() {
        bail!("failed to read current macOS keyboard input source");
    }
    let current_id = input_source_id(source);
    unsafe { CFRelease(source as CFTypeRef) };
    Ok(current_id)
}

fn with_input_source_list<T>(f: impl FnOnce(CFArrayRef, isize) -> Result<T>) -> Result<T> {
    let properties = make_selectable_keyboard_source_filter()?;
    let list = unsafe { TISCreateInputSourceList(properties as CFDictionaryRef, 0) };
    unsafe { CFRelease(properties as CFTypeRef) };
    if list.is_null() {
        bail!("failed to list macOS input sources");
    }

    let count = unsafe { CFArrayGetCount(list) };
    let result = f(list, count);
    unsafe { CFRelease(list as CFTypeRef) };
    result
}

fn make_selectable_keyboard_source_filter() -> Result<CFMutableDictionaryRef> {
    let properties = unsafe {
        CFDictionaryCreateMutable(
            ptr::null(),
            0,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        )
    };
    if properties.is_null() {
        bail!("failed to create macOS input source filter");
    }

    unsafe {
        CFDictionarySetValue(
            properties,
            kTISPropertyInputSourceIsSelectCapable as *const c_void,
            kCFBooleanTrue,
        );
        CFDictionarySetValue(
            properties,
            kTISPropertyInputSourceCategory as *const c_void,
            kTISCategoryKeyboardInputSource as *const c_void,
        );
    }

    Ok(properties)
}

fn input_source_at(list: CFArrayRef, idx: isize) -> TISInputSourceRef {
    unsafe { CFArrayGetValueAtIndex(list, idx) as TISInputSourceRef }
}

fn input_source_id(source: TISInputSourceRef) -> Option<String> {
    string_property(source, unsafe { kTISPropertyInputSourceID })
}

fn string_property(source: TISInputSourceRef, key: CFStringRef) -> Option<String> {
    let value = unsafe { TISGetInputSourceProperty(source, key) as CFStringRef };
    if value.is_null() {
        None
    } else {
        Some(unsafe { CFString::wrap_under_get_rule(value) }.to_string())
    }
}

fn bool_property(source: TISInputSourceRef, key: CFStringRef) -> bool {
    let value = unsafe { TISGetInputSourceProperty(source, key) as CFTypeRef };
    if value.is_null() {
        return false;
    }
    let value = unsafe { CFType::wrap_under_get_rule(value) };
    value
        .downcast::<CFBoolean>()
        .map(bool::from)
        .unwrap_or(false)
}

#[derive(Debug)]
struct InputSourceInfo {
    id: Option<String>,
    input_mode_id: Option<String>,
    localized_name: Option<String>,
    category: Option<String>,
    is_enabled: bool,
    is_select_capable: bool,
}

impl InputSourceInfo {
    fn new(source: TISInputSourceRef) -> Self {
        Self {
            id: input_source_id(source),
            input_mode_id: string_property(source, unsafe { kTISPropertyInputModeID }),
            localized_name: string_property(source, unsafe { kTISPropertyLocalizedName }),
            category: string_property(source, unsafe { kTISPropertyInputSourceCategory }),
            is_enabled: bool_property(source, unsafe { kTISPropertyInputSourceIsEnabled }),
            is_select_capable: bool_property(source, unsafe {
                kTISPropertyInputSourceIsSelectCapable
            }),
        }
    }
}

impl fmt::Display for InputSourceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "input_source_id={:?}, localized_name={:?}, input_mode_id={:?}, category={:?}, isEnabled={}, isSelectCapable={}",
            self.id,
            self.localized_name,
            self.input_mode_id,
            self.category,
            self.is_enabled,
            self.is_select_capable
        )
    }
}

enum HelperResponse {
    Ok(Option<String>),
    Err(String),
}

struct SocketCleanup {
    path: PathBuf,
}

impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        if let Some(socket_dir) = self.path.parent() {
            let _ = fs::remove_dir(socket_dir);
        }
    }
}

fn format_matches(matches: &[(TISInputSourceRef, InputSourceInfo)]) -> String {
    matches
        .iter()
        .map(|(_, info)| info.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_info_refs(infos: &[&InputSourceInfo]) -> String {
    if infos.is_empty() {
        "none".to_owned()
    } else {
        infos
            .iter()
            .map(|info| info.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_field(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }

        match chars.next() {
            Some('\\') => result.push('\\'),
            Some('t') => result.push('\t'),
            Some('n') => result.push('\n'),
            Some(ch) => bail!("invalid escape sequence in helper field: \\{ch}"),
            None => bail!("trailing escape in helper field"),
        }
    }
    Ok(result)
}
