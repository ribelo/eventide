use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn emit_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut event_bus_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "EventBus" {
                    event_bus_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let event_bus_field = event_bus_field.expect("No field of type <EventBus> found");

    let expanded = quote! {
        impl EmitEvent for #name {
            fn event_bus(&self) -> &EventBus {
                &self.#event_bus_field
            }
        }
    };

    TokenStream::from(expanded)
}

pub fn subscribe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl Subscribe for #name {}
    };

    TokenStream::from(expanded)
}

pub fn unsubscribe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl Unsubscribe for #name {}
    };

    TokenStream::from(expanded)
}
