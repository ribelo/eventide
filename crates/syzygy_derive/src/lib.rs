use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

#[proc_macro_derive(ModelAccess)]
pub fn derive_model_access(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let mut models_field = None;
    if let syn::Data::Struct(data) = input.data {
        for field in data.fields {
            if let Type::Path(path) = &field.ty {
                if path.path.segments.last().unwrap().ident == "Models" {
                    models_field = Some(field.ident);
                    break;
                }
            }
        }
    }

    let models_field = models_field.expect("No field of type <Models> found");

    let expanded = quote! {
        impl ModelAccess for #name {
            fn models(&self) -> &Models {
                &self.#models_field
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(ModelMut)]
pub fn derive_model_mut(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl ModelMut for #name {

        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(ResourceAccess)]
pub fn derive_resource_access(input: TokenStream) -> TokenStream {
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

#[proc_macro_derive(DispatchEffect)]
pub fn derive_dispatch_effect(input: TokenStream) -> TokenStream {
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

#[proc_macro_derive(EmitEvent)]
pub fn derive_emit_event(input: TokenStream) -> TokenStream {
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

#[proc_macro_derive(Subscribe)]
pub fn derive_subscribe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl Subscribe for #name {}
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(Unsubscribe)]
pub fn derive_unsubscribe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl Unsubscribe for #name {}
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(SpawnThread)]
pub fn derive_spawn_thread(input: TokenStream) -> TokenStream {
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

#[proc_macro_derive(SpawnAsync)]
pub fn derive_spawn_async(input: TokenStream) -> TokenStream {
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

#[proc_macro_derive(SpawnParallel)]
pub fn derive_spawn_parallel(input: TokenStream) -> TokenStream {
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
