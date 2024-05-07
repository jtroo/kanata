use crate::cfg::sexpr::Span;

type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

#[derive(Debug, Default)]
pub struct LspHints {
    pub inactive_code: Vec<InactiveCode>,
    pub definition_locations: DefinitionLocations,
    pub reference_locations: ReferenceLocations,
}

#[derive(Debug, Clone)]
pub struct InactiveCode {
    pub span: Span,
    pub reason: String,
}

#[derive(Debug, Default, Clone)]
pub struct DefinitionLocations {
    pub alias: HashMap<String, Span>,
    pub variable: HashMap<String, Span>,    // TODO
    pub virtual_key: HashMap<String, Span>, // TODO
    pub chord_group: HashMap<String, Span>, // TODO
    pub layer: HashMap<String, Span>,
}

#[derive(Debug, Default, Clone)]
pub struct ReferenceLocations {
    pub alias: HashMap<String, Vec<Span>>,
    pub variable: HashMap<String, Vec<Span>>,    // TODO
    pub virtual_key: HashMap<String, Vec<Span>>, // TODO
    pub chord_group: HashMap<String, Vec<Span>>, // TODO
    pub layer: HashMap<String, Vec<Span>>,
    pub include: HashMap<String, Vec<Span>>,
}
