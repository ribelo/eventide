use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn resource_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut resources_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "Resources" {
                    resources_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let resources_field = resources_field.expect("No field of type <Resources> found");

    let expanded = quote! {
        impl ResourceAccess for #name {
            fn resources(&self) -> &Resources {
                &self.#resources_field
            }
        }
    };

    TokenStream::from(expanded)
}
