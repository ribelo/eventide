use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn model_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    // Find the first field of type Models
    let mut models_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if matches!(&field.ty, Type::Path(path) if path.path.segments.last().map_or(false, |seg| seg.ident == "Models")) {
                models_field = Some(field.ident);
                break;
            }
        }
    }

    let models_field = models_field.expect("No field of type Models found");

    let expanded = quote! {
        impl ModelAccess for #name {
            fn models(&self) -> &Models {
                &self.#models_field
            }
        }
    };

    TokenStream::from(expanded)
}

pub fn model_mut(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl ModelMut for #name {

        }
    };

    TokenStream::from(expanded)
}
