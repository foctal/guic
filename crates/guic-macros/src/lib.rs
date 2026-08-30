//! Helper macros for GUIC.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, LitStr, Token, parse::Parse, parse::ParseStream, parse_macro_input,
    punctuated::Punctuated,
};

/// Converts a string literal into a `gpui::SharedString`.
///
/// This keeps call sites concise when component builders expect a shared string.
#[proc_macro]
pub fn shared_string(input: TokenStream) -> TokenStream {
    let literal = parse_macro_input!(input as LitStr);
    quote!(::gpui::SharedString::from(#literal)).into()
}

/// Formats values into a `gpui::SharedString`.
///
/// This macro mirrors `format!` but returns `SharedString` directly.
#[proc_macro]
pub fn shared_format(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input with Punctuated::<Expr, Token![,]>::parse_terminated);
    quote!(::gpui::SharedString::from(format!(#args))).into()
}

struct ThemeNameInput {
    name: LitStr,
}

impl Parse for ThemeNameInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            name: input.parse()?,
        })
    }
}

/// Converts a string literal into a `guic_tokens::ThemeName`.
#[proc_macro]
pub fn theme_name(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ThemeNameInput);
    let name = input.name;
    quote!(::guic_tokens::ThemeName::from(#name)).into()
}
