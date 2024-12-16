use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn model_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl syzygy::syzygy_core::model::ModelAccess for #name {
            fn models(&self) -> &syzygy::syzygy_core::model::Models {
                &self.syzygy.models
            }
        }
    };

    TokenStream::from(expanded)
}

pub fn model_mut(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl syzygy::syzygy_core::model::ModelMut for #name {}
    };

    TokenStream::from(expanded)
}
