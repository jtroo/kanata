extern crate proc_macro;
use proc_macro2::{Delimiter, Group, Literal, Punct, Spacing, TokenStream, TokenTree};
use proc_macro_error::proc_macro_error;
use proc_macro_error::{abort, emit_error};
use quote::quote;

#[proc_macro_error]
#[proc_macro]
pub fn layout(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: TokenStream = input.into();

    let mut out = TokenStream::new();

    let mut inside = TokenStream::new();

    for t in input {
        match t {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => {
                let layer = parse_layer(g.stream());
                inside.extend(quote! {
                    [#layer],
                });
            }
            _ => abort!(t, "Invalid token, expected layer: {{ ... }}"),
        }
    }

    let all: TokenStream = quote! { [#inside] };
    out.extend(all);

    out.into()
}

fn parse_layer(input: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for t in input {
        match t {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Bracket => {
                let row = parse_row(g.stream());
                out.extend(quote! {
                    [#row],
                });
            }
            TokenTree::Punct(p) if p.as_char() == ',' => (),
            _ => abort!(t, "Invalid token, expected row: [ ... ]"),
        }
    }
    out
}

fn parse_row(input: TokenStream) -> TokenStream {
    let mut out = TokenStream::new();
    for t in input {
        match t {
            TokenTree::Ident(i) => match i.to_string().as_str() {
                "n" => out.extend(quote! { keyberon::action::Action::NoOp, }),
                "t" => out.extend(quote! { keyberon::action::Action::Trans, }),
                _ => out.extend(quote! {
                    keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::#i),
                }),
            },
            TokenTree::Punct(p) => punctuation_to_keycode(&p, &mut out),
            TokenTree::Literal(l) => literal_to_keycode(&l, &mut out),
            TokenTree::Group(g) => parse_group(&g, &mut out),
        }
    }
    out
}

fn parse_group(g: &Group, out: &mut TokenStream) {
    match g.delimiter() {
        // Handle empty groups
        Delimiter::Parenthesis if g.stream().is_empty() => {
            emit_error!(g, "Expected a layer number in layer switch"; help = "To create a parenthesis keycode, enclose it in apostrophes: '('")
        }
        Delimiter::Brace if g.stream().is_empty() => {
            emit_error!(g, "Expected an action - group cannot be empty"; help = "To create a brace keycode, enclose it in apostrophes: '{'")
        }
        Delimiter::Bracket if g.stream().is_empty() => {
            emit_error!(g, "Expected keycodes - keycode group cannot be empty"; help = "To create a bracket keycode, enclose it in apostrophes: '['")
        }

        // Momentary layer switch (Action::Layer)
        Delimiter::Parenthesis => {
            let tokens = g.stream();
            out.extend(quote! { keyberon::action::Action::Layer(#tokens), });
        }
        // Pass the expression unchanged (adding a comma after it)
        Delimiter::Brace => out.extend(g.stream().into_iter().chain(TokenStream::from(
            TokenTree::Punct(Punct::new(',', Spacing::Alone)),
        ))),
        // Multiple keycodes (Action::MultipleKeyCodes)
        Delimiter::Bracket => parse_keycode_group(g.stream(), out),

        // Is this reachable?
        Delimiter::None => emit_error!(g, "Unexpected group"),
    }
}

fn parse_keycode_group(input: TokenStream, out: &mut TokenStream) {
    let mut inner = TokenStream::new();
    for t in input {
        match t {
            TokenTree::Ident(i) => inner.extend(quote! {
                keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::#i),
            }),
            TokenTree::Punct(p) => punctuation_to_keycode(&p, &mut inner),
            TokenTree::Literal(l) => literal_to_keycode(&l, &mut inner),
            TokenTree::Group(g) => parse_group(&g, &mut inner),
        }
    }
    out.extend(quote! { keyberon::action::Action::MultipleActions(&[#inner].as_slice()), });
}

fn punctuation_to_keycode(p: &Punct, out: &mut TokenStream) {
    match p.as_char() {
        // Normal punctuation
        '-' => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Minus), }),
        '=' => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Equal), }),
        ';' => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::SColon), }),
        ',' => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Comma), }),
        '.' => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Dot), }),
        '/' => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Slash), }),

        // Shifted punctuation
        '!' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb1].as_slice()), }),
        '@' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb2].as_slice()), }),
        '#' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb3].as_slice()), }),
        '$' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb4].as_slice()), }),
        '%' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb5].as_slice()), }),
        '^' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb6].as_slice()), }),
        '&' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb7].as_slice()), }),
        '*' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb8].as_slice()), }),
        '_' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Minus].as_slice()), }),
        '+' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Equal].as_slice()), }),
        '|' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Bslash].as_slice()), }),
        '~' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Grave].as_slice()), }),
        '<' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Comma].as_slice()), }),
        '>' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Dot].as_slice()), }),
        '?' => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Slash].as_slice()), }),
        // Is this reachable?
        _ => emit_error!(p, "Punctuation could not be parsed as a keycode")
    }
}

fn literal_to_keycode(l: &Literal, out: &mut TokenStream) {
    //let repr = l.to_string();
    match l.to_string().as_str() {
        "1" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb1), }),
        "2" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb2), }),
        "3" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb3), }),
        "4" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb4), }),
        "5" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb5), }),
        "6" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb6), }),
        "7" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb7), }),
        "8" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb8), }),
        "9" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb9), }),
        "0" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Kb0), }),

        // Char literals; mostly punctuation which can't be properly tokenized alone
        r#"'\''"# => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Quote), }),
        r#"'\\'"# => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Bslash), }),
        // Shifted characters
        "'['" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::LBracket), }),
        "']'" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::RBracket), }),
        "'`'" => out.extend(quote! { keyberon::action::Action::KeyCode(keyberon::key_code::KeyCode::Grave), }),
        "'\"'" => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Quote].as_slice()), }),
        "'('" => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb9].as_slice()), }),
        "')'" => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Kb0].as_slice()), }),
        "'{'" => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::LBracket].as_slice()), }),
        "'}'" => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::RBracket].as_slice()), }),
        "'_'" => out.extend(quote! { keyberon::action::Action::MultipleKeyCodes(&[keyberon::key_code::KeyCode::LShift, keyberon::key_code::KeyCode::Minus].as_slice()), }),

        s if s.starts_with('\'') => emit_error!(l, "Literal could not be parsed as a keycode"; help = "Maybe try without quotes?"),

        s if s.starts_with('\"')  => {
            if s.len() == 3 {
                emit_error!(l, "Typing strings on key press is not yet supported"; help = "Did you mean to use apostrophes instead of quotes?");
            } else {
                emit_error!(l, "Typing strings on key press is not yet supported");
            }
        }
        _ => emit_error!(l, "Literal could not be parsed as a keycode")
    }
}
