use crate::cfg::sexpr::{Span, Spanned};

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
    pub variable: HashMap<String, Span>, // TODO
    pub virtual_key: HashMap<String, Span>,
    pub chord_group: HashMap<String, Span>, // TODO
    pub layer: HashMap<String, Span>,
}

#[derive(Debug, Default, Clone)]
pub struct ReferenceLocations {
    pub alias: ReferencesMap,
    pub variable: ReferencesMap, // TODO
    pub virtual_key: ReferencesMap,
    pub chord_group: ReferencesMap, // TODO
    pub layer: ReferencesMap,
    pub include: ReferencesMap,
}

#[derive(Debug, Default, Clone)]
pub struct ReferencesMap(pub HashMap<String, Vec<Span>>);

impl ReferencesMap {
    pub(crate) fn push_from_atom(&mut self, atom: &Spanned<String>) {
        match self.0.get_mut(&atom.t) {
            Some(refs) => refs.push(atom.span.clone()),
            None => {
                self.0.insert(atom.t.clone(), vec![atom.span.clone()]);
            }
        };
    }
}
