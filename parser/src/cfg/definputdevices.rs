use super::*;
use crate::{anyhow_expr, bail_expr};
use std::num::NonZeroU8;

#[derive(Debug, Clone, Default)]
pub struct InputDeviceMatcher {
    pub name: Option<String>,
    pub hash: Option<String>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
}

pub fn parse_definputdevices(expr: &[SExpr]) -> Result<Vec<(NonZeroU8, InputDeviceMatcher)>> {
    let mut exprs = check_first_expr(expr.iter(), "definputdevices")?;
    let mut seen_ids = HashSet::default();
    let mut devices = vec![];
    loop {
        let Some(id_expr) = exprs.next() else {
            break;
        };
        let Some(matchers_expr) = exprs.next() else {
            bail_expr!(
                id_expr,
                "definputdevices expects pairs: <id> <matcher-list>.\n\
                 Missing matcher list for this device ID."
            );
        };
        let id_str = id_expr
            .atom(None)
            .ok_or_else(|| anyhow_expr!(id_expr, "device ID must be a number (1-255)"))?;
        let id_num: u8 = id_str
            .parse()
            .map_err(|_| anyhow_expr!(id_expr, "device ID must be a number (1-255)"))?;
        let id = NonZeroU8::new(id_num)
            .ok_or_else(|| anyhow_expr!(id_expr, "device ID must be nonzero (1-255)"))?;
        if !seen_ids.insert(id) {
            bail_expr!(id_expr, "duplicate device ID: {id_num}");
        }
        let matcher_list = matchers_expr
            .list(None)
            .ok_or_else(|| anyhow_expr!(matchers_expr, "device matchers must be a list"))?;
        if matcher_list.is_empty() {
            bail_expr!(
                matchers_expr,
                "device matcher list must not be empty; \
                 specify at least one of: name, hash, vendor_id, product_id"
            );
        }
        let mut matcher = InputDeviceMatcher::default();
        for m in matcher_list.iter() {
            let props = m.list(None).ok_or_else(|| {
                anyhow_expr!(m, "each matcher must be a list, e.g. (name \"...\")")
            })?;
            if props.len() != 2 {
                bail_expr!(
                    m,
                    "each matcher must have exactly 2 items: (property value)"
                );
            }
            let prop_name = props[0]
                .atom(None)
                .ok_or_else(|| anyhow_expr!(&props[0], "matcher property must be an atom"))?;
            let prop_val = props[1]
                .atom(None)
                .ok_or_else(|| anyhow_expr!(&props[1], "matcher value must be an atom"))?;
            match prop_name {
                "name" => {
                    if matcher.name.is_some() {
                        bail_expr!(m, "duplicate property: name");
                    }
                    matcher.name = Some(prop_val.to_string());
                }
                "hash" => {
                    if matcher.hash.is_some() {
                        bail_expr!(m, "duplicate property: hash");
                    }
                    let stripped = prop_val
                        .strip_prefix("0x")
                        .or_else(|| prop_val.strip_prefix("0X"))
                        .unwrap_or(prop_val);
                    if stripped.is_empty() || !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
                        bail_expr!(
                            &props[1],
                            "hash must be a valid hex string (e.g. \"a1b2c3def4\")"
                        );
                    }
                    // Store lowercase, without 0x prefix, for case-insensitive matching.
                    matcher.hash = Some(stripped.to_ascii_lowercase());
                }
                "vendor_id" => {
                    if matcher.vendor_id.is_some() {
                        bail_expr!(m, "duplicate property: vendor_id");
                    }
                    let v = parse_hex_or_decimal_u16(prop_val).map_err(|_| {
                        anyhow_expr!(
                            &props[1],
                            "vendor_id must be a number 0-65535 or hex (e.g. 0x1D50)"
                        )
                    })?;
                    matcher.vendor_id = Some(v);
                }
                "product_id" => {
                    if matcher.product_id.is_some() {
                        bail_expr!(m, "duplicate property: product_id");
                    }
                    let v = parse_hex_or_decimal_u16(prop_val).map_err(|_| {
                        anyhow_expr!(
                            &props[1],
                            "product_id must be a number 0-65535 or hex (e.g. 0x615E)"
                        )
                    })?;
                    matcher.product_id = Some(v);
                }
                _ => {
                    bail_expr!(
                        &props[0],
                        "unknown matcher property: {prop_name}\n\
                         valid properties: name, hash, vendor_id, product_id"
                    );
                }
            }
        }
        devices.push((id, matcher));
    }
    Ok(devices)
}

fn parse_hex_or_decimal_u16(s: &str) -> std::result::Result<u16, Box<dyn std::error::Error>> {
    let val: u64 = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16)?
    } else {
        s.parse()?
    };
    u16::try_from(val).map_err(|e| e.into())
}
