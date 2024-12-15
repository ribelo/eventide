use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn dispatch_effect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut dispatcher_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "Dispatcher" {
                    dispatcher_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let dispatcher_field = dispatcher_field.expect("No field of type <Dispatcher> found");

    let expanded = quote! {
        impl DispatchEffect for #name {
            fn dispatcher(&self) -> &Dispatcher {
                &self.#dispatcher_field
            }
        }
    };

    TokenStream::from(expanded)
}
