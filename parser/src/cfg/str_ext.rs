pub trait TrimAtomQuotes {
    fn trim_atom_quotes(&self) -> &str;
}

impl TrimAtomQuotes for str {
    fn trim_atom_quotes(&self) -> &str {
        match self.strip_prefix("r#\"") {
            Some(a) => a.strip_suffix("\"#").unwrap_or(a),
            None => self
                .strip_prefix('"')
                .unwrap_or(self)
                .strip_suffix('"')
                .unwrap_or(self),
        }
    }
}

impl TrimAtomQuotes for String {
    fn trim_atom_quotes(&self) -> &str {
        match self.as_str().strip_prefix("r#\"") {
            Some(a) => a.strip_suffix("\"#").unwrap_or(a),
            None => self
                .strip_prefix('"')
                .unwrap_or(self)
                .strip_suffix('"')
                .unwrap_or(self),
        }
    }
}
