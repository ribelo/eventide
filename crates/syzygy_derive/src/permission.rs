use darling::{ast, FromDeriveInput, FromField, FromMeta};
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Type};

pub fn role(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident.clone();

    let expanded = quote! {
        impl Role for #name {}
    };

    TokenStream::from(expanded)
}

#[allow(dead_code)]
#[derive(FromDeriveInput)]
#[darling(attributes(syzygy))]
struct RoleOpts {
    role: syn::Path,
}

pub fn grant_role(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident.clone();

    let opts = RoleOpts::from_derive_input(&input)
        .expect("PermissionHolder requires #[permission(ty = \"Type\")] attribute");

    let permission_type = opts.role;

    let expanded = quote! {
        impl RoleHolder for #name {
            type Role = #permission_type;
        }
    };

    TokenStream::from(expanded)
}

pub fn guard_role(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident.clone();

    let opts = RoleOpts::from_derive_input(&input)
        .expect("PermissionGuarded requires #[permission(ty = \"Type\")] attribute");

    let permission_type = opts.role;

    let expanded = quote! {
        impl RoleGuarded for #name {
            type Role = #permission_type;
        }
    };

    TokenStream::from(expanded)
}

#[allow(dead_code)]
#[derive(FromDeriveInput)]
#[darling(attributes(syzygy), forward_attrs(allow, doc, cfg))]
struct ImpliedByOpts {
    #[darling(multiple)]
    implied_by: Vec<syn::Path>,
}

pub fn implied_by(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident.clone();

    let opts = match ImpliedByOpts::from_derive_input(&input) {
        Ok(opts) => opts,
        Err(err) => {
            dbg!(&err);
            return TokenStream::from(quote! {
                compile_error!(format!("ImpliedBy error: {:?}", err));
            });
        }
    };

    let permission_type = opts.implied_by;

    let expanded = quote! {
        #(impl ImpliedBy<#permission_type> for #name {})*
    };

    TokenStream::from(expanded)
}
