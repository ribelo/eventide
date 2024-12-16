use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn spawn_thread(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl syzygy::syzygy_core::spawn::SpawnThread for #name {}
    };

    TokenStream::from(expanded)
}

pub fn spawn_async(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl syzygy::syzygy_core::spawn::SpawnAsync for #name {
            fn tokio_rt(&self) -> &tokio::runtime::Runtime {
                &self.syzygy.tokio_rt
            }
        }
        impl syzygy::syzygy_core::context::FromContext<#name> for syzygy::syzygy_core::context::r#async::AsyncContext {
            fn from_context(cx: #name) -> Self {
                syzygy::syzygy_core::context::FromContext::from_context(&cx.syzygy)
            }
        }
    };

    TokenStream::from(expanded)
}

pub fn spawn_parallel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut task_queue_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "ParallelTaskQueue" {
                    task_queue_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let task_queue_field = task_queue_field.expect("No field of type <ParallelTaskQueue> found");

    let expanded = quote! {
        impl SpawnParallel for #name {
            fn parallel_task_queue(&self) -> &ParallelTaskQueue {
                &self.#task_queue_field
            }
        }
    };

    TokenStream::from(expanded)
}
