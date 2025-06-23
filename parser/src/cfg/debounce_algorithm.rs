

#[cfg(any(target_os = "linux", target_os = "unknown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebounceAlgorithm {
    AsymEagerDeferPk,
    SymEagerPk,
    SymDeferPk,
}

#[cfg(any(target_os = "linux", target_os = "unknown"))]
impl std::str::FromStr for DebounceAlgorithm {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "asym_eager_defer_pk" => Ok(DebounceAlgorithm::AsymEagerDeferPk),
            "sym_eager_pk" => Ok(DebounceAlgorithm::SymEagerPk),
            "sym_defer_pk" => Ok(DebounceAlgorithm::SymDeferPk),
            _ => Err(format!("Unknown debounce algorithm: {}", s)),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "unknown"))]
impl std::fmt::Display for DebounceAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let algorithm_name = match self {
            DebounceAlgorithm::AsymEagerDeferPk => "asym_eager_defer_pk",
            DebounceAlgorithm::SymEagerPk => "sym_eager_pk",
            DebounceAlgorithm::SymDeferPk => "sym_defer_pk",
        };
        write!(f, "{}", algorithm_name)
    }
}
