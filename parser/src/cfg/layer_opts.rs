use crate::cfg::*;
use crate::*;

pub(crate) const DEFLAYER_ICON: [&str; 3] = ["icon", "🖻", "🖼"];
pub(crate) type LayerIcons = HashMap<String, Option<String>>;

pub fn parse_layer_opts(list: &[SExpr]) -> Result<HashMap<String, String>> {
    let mut layer_opts: HashMap<String, String> = HashMap::default();
    let mut opts = list.chunks_exact(2);
    for kv in opts.by_ref() {
        let key_expr = &kv[0];
        let val_expr = &kv[1];
        // Read k-v pairs from the configuration
        // todo: add hashmap for future options, currently only parse icons
        let opt_key = key_expr.atom(None)
            .ok_or_else(|| anyhow_expr!(key_expr, "No lists are allowed in {DEFLAYER} options"))
            .and_then(|opt_key| {
                if DEFLAYER_ICON.contains(&opt_key) {
                    if layer_opts.contains_key(DEFLAYER_ICON[0]) {
                        // separate dupe check since multi-keys are stored
                        // with one "canonical" repr, so '🖻' → 'icon'
                        // and this info will be lost after the loop
                        bail_expr!(
                            key_expr,
                            "Duplicate option found in {DEFLAYER}: {opt_key}, one of {DEFLAYER_ICON:?} already exists"
                        );
                    }
                    Ok(DEFLAYER_ICON[0])
                } else {
                    bail_expr!(key_expr, "Invalid option in {DEFLAYER}: {opt_key}, expected one of {DEFLAYER_ICON:?}")
                }
            })?;
        if layer_opts.contains_key(opt_key) {
            bail_expr!(key_expr, "Duplicate option found in {DEFLAYER}: {opt_key}");
        }
        let opt_val = val_expr.atom(None).ok_or_else(|| {
            anyhow_expr!(
                val_expr,
                "No lists are allowed in {DEFLAYER}'s option values"
            )
        })?;
        layer_opts.insert(opt_key.to_owned(), opt_val.to_owned());
    }
    let rem = opts.remainder();
    if !rem.is_empty() {
        bail_expr!(&rem[0], "This option is missing a value.");
    }
    Ok(layer_opts)
}
