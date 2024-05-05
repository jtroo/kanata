use crate::cfg::*;
use crate::*;

pub(crate) const DEFLAYER_ICON: [&str; 3] = ["icon", "🖻", "🖼"];
pub(crate) type LayerIcons = HashMap<String, Option<String>>;

pub fn parse_layer_opts(list: &[SExpr]) -> Result<HashMap<String, String>> {
    let mut layer_opts: HashMap<String, String> = HashMap::default();
    let mut list = list.iter();
    while let Some(key_expr) = list.next() {
        // Read k-v pairs from the configuration
        // todo: add hashmap for future options, currently only parse icons
        let opt_key = key_expr.atom(None).ok_or_else(|| {anyhow_expr!(key_expr, "No lists are allowed in {DEFLAYER} options")}).and_then(|opt_key| {
                if DEFLAYER_ICON.iter().any(|&i| i == opt_key) {
                    if layer_opts.contains_key(DEFLAYER_ICON[0]) {
                        bail_expr!(key_expr,"Duplicate option found in {DEFLAYER}: {opt_key}, one of {DEFLAYER_ICON:?} already exists");}
                        // separate dupe check since multi-keys are stored with one "canonical" repr, so '🖻' → 'icon'
                        // and this info will be lost after the loop
                    Ok(DEFLAYER_ICON[0])
                } else {bail_expr!(key_expr, "Invalid option in {DEFLAYER}: {opt_key}, expected one of {DEFLAYER_ICON:?}")}
                })?;
        if layer_opts.contains_key(opt_key) {
            bail_expr!(key_expr, "Duplicate option found in {DEFLAYER}: {opt_key}");
        }
        let opt_val = match list.next() {
            Some(v) => v.atom(None).ok_or_else(|| {
                anyhow_expr!(v, "No lists are allowed in {DEFLAYER}'s option values")
            })?,
            None => {
                bail_expr!(key_expr, "Option without a value in {DEFLAYER}")
            }
        };
        layer_opts.insert(opt_key.to_owned(), opt_val.to_owned());
    }
    Ok(layer_opts)
}
