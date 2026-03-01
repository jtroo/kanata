use super::*;

use crate::anyhow_expr;
use crate::bail_expr;

#[cfg(all(
    not(feature = "interception_driver"),
    any(
        not(feature = "win_llhook_read_scancodes"),
        not(feature = "win_sendinput_send_scancodes")
    ),
    target_os = "windows"
))]
pub(crate) const DEF_LOCAL_KEYS: &str = "deflocalkeys-win";
#[cfg(all(
    feature = "win_llhook_read_scancodes",
    feature = "win_sendinput_send_scancodes",
    not(feature = "interception_driver"),
    target_os = "windows"
))]
pub(crate) const DEF_LOCAL_KEYS: &str = "deflocalkeys-winiov2";
#[cfg(all(feature = "interception_driver", target_os = "windows"))]
pub(crate) const DEF_LOCAL_KEYS: &str = "deflocalkeys-wintercept";
#[cfg(target_os = "macos")]
pub(crate) const DEF_LOCAL_KEYS: &str = "deflocalkeys-macos";
#[cfg(any(target_os = "linux", target_os = "android", target_os = "unknown"))]
pub(crate) const DEF_LOCAL_KEYS: &str = "deflocalkeys-linux";

pub(crate) fn deflocalkeys_variant_applies_to_current_os(variant: &str) -> bool {
    variant == DEF_LOCAL_KEYS
}

pub(crate) const DEFLOCALKEYS_VARIANTS: &[&str] = &[
    "deflocalkeys-win",
    "deflocalkeys-winiov2",
    "deflocalkeys-wintercept",
    "deflocalkeys-linux",
    "deflocalkeys-macos",
];

/// Parse custom keys from an expression starting with deflocalkeys.
pub(crate) fn parse_deflocalkeys(
    def_local_keys_variant: &str,
    expr: &[SExpr],
) -> Result<HashMap<String, OsCode>> {
    let mut localkeys = HashMap::default();
    let mut exprs = check_first_expr(expr.iter(), def_local_keys_variant)?;
    // Read k-v pairs from the configuration
    while let Some(key_expr) = exprs.next() {
        let key = key_expr.atom(None).ok_or_else(|| {
            anyhow_expr!(key_expr, "No lists are allowed in {def_local_keys_variant}")
        })?;
        if localkeys.contains_key(key) {
            bail_expr!(
                key_expr,
                "Duplicate {key} found in {def_local_keys_variant}"
            );
        }

        // Bug:
        // Trying to convert a number to OsCode is OS-dependent and is fallible.
        // A valid number for Linux could throw an error on Windows.
        //
        // Fix:
        // When the deflocalkeys variant does not apply to the current OS,
        // use a dummy OsCode to keep the "same name" validation
        // while avoiding the u16->OsCode conversion attempt.
        if !deflocalkeys_variant_applies_to_current_os(def_local_keys_variant) {
            localkeys.insert(key.to_owned(), OsCode::KEY_RESERVED);
            continue;
        }

        let osc = match exprs.next() {
            Some(v) => v
                .atom(None)
                .ok_or_else(|| anyhow_expr!(v, "No lists are allowed in {def_local_keys_variant}"))
                .and_then(|osc| {
                    osc.parse::<u16>().map_err(|_| {
                        anyhow_expr!(v, "Unknown number in {def_local_keys_variant}: {osc}")
                    })
                })
                .and_then(|osc| {
                    OsCode::from_u16(osc).ok_or_else(|| {
                        anyhow_expr!(v, "Unknown number in {def_local_keys_variant}: {osc}")
                    })
                })?,
            None => bail_expr!(key_expr, "Key without a number in {def_local_keys_variant}"),
        };
        log::debug!("custom mapping: {key} {}", osc.as_u16());
        localkeys.insert(key.to_owned(), osc);
    }
    Ok(localkeys)
}
