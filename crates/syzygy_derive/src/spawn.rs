use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn spawn_thread(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut task_queue_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "TaskQueue" {
                    task_queue_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let task_queue_field = task_queue_field.expect("No field of type <TaskQueue> found");

    let expanded = quote! {
        impl SpawnThread for #name {
            fn task_queue(&self) -> &TaskQueue {
                &self.#task_queue_field
            }
        }
    };

    TokenStream::from(expanded)
}

pub fn spawn_async(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut task_queue_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "AsyncTaskQueue" {
                    task_queue_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let task_queue_field = task_queue_field.expect("No field of type <AsyncTaskQueue> found");

    let expanded = quote! {
        impl SpawnAsync for #name {
            fn async_task_queue(&self) -> &AsyncTaskQueue {
                &self.#task_queue_field
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
