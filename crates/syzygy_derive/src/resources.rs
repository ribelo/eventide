use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn resource_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl syzygy::syzygy_core::resource::ResourceAccess for #name {
            fn resources(&self) -> &syzygy::syzygy_core::resource::Resources {
                &self.syzygy.resources
            }
        }
    };

    TokenStream::from(expanded)
}
