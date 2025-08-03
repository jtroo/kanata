use anyhow::{anyhow, Result};

/// Convert a list of device filter strings to VID/PID pairs, filtering out device names
pub fn sexpr_to_vid_pids_vec(val: &crate::cfg::SExpr, label: &str) -> Result<Vec<(u16, u16)>> {
    use crate::cfg::SExpr;

    let vid_pid_list = match val {
        SExpr::List(l) => &l.t,
        _ => return Err(anyhow!("The value for {label} must be a list")),
    };

    let mut vid_pids = Vec::new();

    for expr in vid_pid_list {
        let vid_pid_str = match expr {
            SExpr::Atom(a) => &a.t,
            SExpr::List(_) => return Err(anyhow!("Entry in {label} must be a string")),
        };

        let vid_pid = parse_vid_pid_string(vid_pid_str)
            .map_err(|e| anyhow!("In {label}: '{}' - {}", vid_pid_str, e))?;
        vid_pids.push(vid_pid);
    }

    vid_pids.shrink_to_fit();
    Ok(vid_pids)
}

/// Parse VID:PID string in decimal format (e.g., "1452:641") to (vendor_id, product_id)
pub fn parse_vid_pid_string(s: &str) -> Result<(u16, u16)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "VID:PID must be in format 'VENDOR_ID:PRODUCT_ID' (e.g., '1452:641'), got: {}",
            s
        ));
    }

    let vendor_id = parts[0]
        .parse::<u16>()
        .map_err(|_| anyhow!("Invalid vendor ID '{}', must be a number 0-65535", parts[0]))?;

    let product_id = parts[1].parse::<u16>().map_err(|_| {
        anyhow!(
            "Invalid product ID '{}', must be a number 0-65535",
            parts[1]
        )
    })?;

    Ok((vendor_id, product_id))
}

/// VID/PID filtering logic for device inclusion/exclusion
pub fn should_include_device_by_vid_pid(
    device_vid_pid: Option<(u16, u16)>,
    include_vid_pids: Option<&[(u16, u16)]>,
    exclude_vid_pids: Option<&[(u16, u16)]>,
) -> bool {
    // Include filter logic
    let include_match = match include_vid_pids {
        None => true, // No include filter means include all
        Some(include_list) => {
            if let Some(device_vid_pid) = device_vid_pid {
                include_list.contains(&device_vid_pid)
            } else {
                false // If we can't extract VID/PID but include filter is specified, reject
            }
        }
    };

    // Exclude filter logic
    let exclude_match = match exclude_vid_pids {
        None => false, // No exclude filter means exclude nothing
        Some(exclude_list) => {
            if let Some(device_vid_pid) = device_vid_pid {
                exclude_list.contains(&device_vid_pid)
            } else {
                false // If we can't extract VID/PID but exclude filter is specified, allow
            }
        }
    };

    // Device is included if it matches include criteria AND doesn't match exclude criteria
    include_match && !exclude_match
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vid_pid_string() {
        assert_eq!(parse_vid_pid_string("1452:641").unwrap(), (1452, 641));
        assert_eq!(parse_vid_pid_string("0:0").unwrap(), (0, 0));
        assert_eq!(parse_vid_pid_string("65535:65535").unwrap(), (65535, 65535));

        // Test error cases
        assert!(parse_vid_pid_string("invalid").is_err());
        assert!(parse_vid_pid_string("1452:").is_err());
        assert!(parse_vid_pid_string(":641").is_err());
        assert!(parse_vid_pid_string("1452:abc").is_err());
        assert!(parse_vid_pid_string("abc:641").is_err());
    }

    #[test]
    fn test_should_include_device_by_vid_pid() {
        let apple_vid_pid = Some((1452, 641));
        let logitech_vid_pid = Some((1133, 49970));
        let unknown_vid_pid = None;

        // No filters - include all
        assert!(should_include_device_by_vid_pid(apple_vid_pid, None, None));
        assert!(should_include_device_by_vid_pid(
            unknown_vid_pid,
            None,
            None
        ));

        // Include filter only
        let include_list = [(1452, 641)];
        assert!(should_include_device_by_vid_pid(
            apple_vid_pid,
            Some(&include_list),
            None
        ));
        assert!(!should_include_device_by_vid_pid(
            logitech_vid_pid,
            Some(&include_list),
            None
        ));
        assert!(!should_include_device_by_vid_pid(
            unknown_vid_pid,
            Some(&include_list),
            None
        ));

        // Exclude filter only
        let exclude_list = [(1452, 641)];
        assert!(!should_include_device_by_vid_pid(
            apple_vid_pid,
            None,
            Some(&exclude_list)
        ));
        assert!(should_include_device_by_vid_pid(
            logitech_vid_pid,
            None,
            Some(&exclude_list)
        ));
        assert!(should_include_device_by_vid_pid(
            unknown_vid_pid,
            None,
            Some(&exclude_list)
        ));

        // Both include and exclude filters
        let include_list = [(1452, 641), (1133, 49970)];
        let exclude_list = [(1452, 641)];
        assert!(!should_include_device_by_vid_pid(
            apple_vid_pid,
            Some(&include_list),
            Some(&exclude_list)
        )); // Excluded
        assert!(should_include_device_by_vid_pid(
            logitech_vid_pid,
            Some(&include_list),
            Some(&exclude_list)
        )); // Included
        assert!(!should_include_device_by_vid_pid(
            unknown_vid_pid,
            Some(&include_list),
            Some(&exclude_list)
        )); // Not in include list
    }
}
