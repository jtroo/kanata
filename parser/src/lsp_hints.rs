pub use inner::*;

#[cfg(not(feature = "lsp"))]
mod inner {
    #[derive(Debug, Default)]
    pub struct LspHints {}
}

#[cfg(feature = "lsp")]
mod inner {
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
        pub variable: HashMap<String, Span>,
        pub virtual_key: HashMap<String, Span>,
        pub layer: HashMap<String, Span>,
        pub template: HashMap<String, Span>,
    }

    #[derive(Debug, Default, Clone)]
    pub struct ReferenceLocations {
        pub alias: ReferencesMap,
        pub variable: ReferencesMap,
        pub virtual_key: ReferencesMap,
        pub layer: ReferencesMap,
        pub template: ReferencesMap,
        pub include: ReferencesMap,
    }

    #[derive(Debug, Default, Clone)]
    pub struct ReferencesMap(pub HashMap<String, Vec<Span>>);

    #[allow(unused)]
    impl ReferencesMap {
        pub(crate) fn push_from_atom(&mut self, atom: &Spanned<String>) {
            match self.0.get_mut(&atom.t) {
                Some(refs) => refs.push(atom.span.clone()),
                None => {
                    self.0.insert(atom.t.clone(), vec![atom.span.clone()]);
                }
            };
        }

        pub(crate) fn push(&mut self, name: &str, span: Span) {
            match self.0.get_mut(name) {
                Some(refs) => refs.push(span),
                None => {
                    self.0.insert(name.to_owned(), vec![span]);
                }
            };
        }
    }
}
