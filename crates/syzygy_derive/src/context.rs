use darling::{
    ast::{self, NestedMeta},
    Error, FromDeriveInput, FromField, FromMeta,
};
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, token::Struct, DeriveInput, ItemFn, Path, Type};

pub fn context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let expanded = quote! {
        impl syzygy::syzygy_core::context::Context for #name {}
    };

    TokenStream::from(expanded)
}

// #[allow(dead_code)]
// #[derive(Debug, FromMeta)]
// struct SyzygyArgs {
//     #[darling(default)]
//     model_access: bool,
//     #[darling(default)]
//     model_mut: bool,
//     #[darling(default)]
//     resource_access: bool,
//     #[darling(default)]
//     dispatch: bool,
//     #[darling(default)]
//     emit: bool,
//     #[darling(default)]
//     spawn_async: bool,
//     #[darling(default)]
//     spawn_paralell: bool,
//     role: Path,
// }

// pub fn syzygy(args: TokenStream, input: TokenStream) -> TokenStream {
//     let attr_args = match NestedMeta::parse_meta_list(args.into()) {
//         Ok(v) => v,
//         Err(e) => {
//             return TokenStream::from(Error::from(e).write_errors());
//         }
//     };
//     let input = syn::parse_macro_input!(input as DeriveInput);
//     let vis = &input.vis;
//     let name = &input.ident;

//     let fields = match input.data {
//         syn::Data::Struct(data) => data.fields.into_iter().collect::<Vec<_>>(),
//         _ => {
//             let err = syn::Error::new_spanned(&input, "only structs are supported");
//             return TokenStream::from(err.to_compile_error());
//         }
//     };

//     let args = match SyzygyArgs::from_list(&attr_args) {
//         Ok(v) => v,
//         Err(e) => {
//             return TokenStream::from(e.write_errors());
//         }
//     };

//     let mut field_tokens = Vec::new();
//     field_tokens.extend(fields.iter().map(|f| quote!(#f)));
//     let mut trait_impl_tokens = Vec::new();

//     let context_trait_impl = generate_context_impl(name.clone());
//     trait_impl_tokens.push(TokenStream2::from(context_trait_impl));
//     let role_holder_trait_impl = generate_role_holder_trait(name.clone(), args.role);
//     trait_impl_tokens.push(TokenStream2::from(role_holder_trait_impl));

//     let clone_trait_impl = quote! {
//         impl Clone for #name {
//         }

//     if args.model_access || args.model_mut {
//         field_tokens.push(quote! {
//             syzygy_models: syzygy_core::model::Models
//         });
//     }

//     if args.model_access {
//         let trait_impl = generate_model_access_trait(
//             name.clone(),
//             syn::Ident::new("syzygy_models", proc_macro2::Span::call_site()),
//         );
//         trait_impl_tokens.push(TokenStream2::from(trait_impl));
//     }

//     if args.model_mut {
//         // trait_impl_tokens.push(quote! {
//         //     #[derive(syzygy_core::model::ModelMut)]
//         // });
//     }

//     if args.resource_access {
//         field_tokens.push(quote! {
//             syzygy_resources: syzygy_core::resource::Resources
//         });
//         // trait_impl_tokens.push(quote! {
//         //     #[derive(syzygy_core::resource::ResourceAccess)]
//         // });
//     }

//     if args.dispatch {
//         field_tokens.push(quote! {
//             syzygy_dispatcher: syzygy_core::dispatch::Dispatcher
//         });
//     }

//     if args.emit {
//         field_tokens.push(quote! {
//             syzygy_event_bus: syzygy_core::event_bus::EventBus
//         });
//     }

//     if args.spawn_async {
//         field_tokens.push(quote! {
//             syzygy_tokio_rt: std::sync::Arc<tokio::runtime::Runtime>
//         });
//     }

//     if args.spawn_paralell {
//         field_tokens.push(quote! {
//             syzygy_parallel: std::sync::Arc<rayon::ThreadPool>
//         });
//     }

//     let output = quote! {
//         #vis struct #name {
//             #(#field_tokens),*
//         }
//         #(#trait_impl_tokens)*

//     };

//     TokenStream::from(output)
// }
