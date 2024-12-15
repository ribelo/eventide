mod dispatch;
mod event_bus;
mod models;
mod permission;
mod resources;
mod spawn;
mod context;

use proc_macro::TokenStream;

#[proc_macro_derive(Context)]
pub fn derive_context(input: TokenStream) -> TokenStream {
    context::context(input)
}

#[proc_macro_derive(ModelAccess)]
pub fn derive_model_access(input: TokenStream) -> TokenStream {
    models::model_access(input)
}

#[proc_macro_derive(ModelMut)]
pub fn derive_model_mut(input: TokenStream) -> TokenStream {
    models::model_mut(input)
}

#[proc_macro_derive(ResourceAccess)]
pub fn derive_resource_access(input: TokenStream) -> TokenStream {
    resources::resource_access(input)
}

#[proc_macro_derive(DispatchEffect)]
pub fn derive_dispatch_effect(input: TokenStream) -> TokenStream {
    dispatch::dispatch_effect(input)
}

#[proc_macro_derive(EmitEvent)]
pub fn derive_emit_event(input: TokenStream) -> TokenStream {
    event_bus::emit_event(input)
}

#[proc_macro_derive(Subscribe)]
pub fn derive_subscribe(input: TokenStream) -> TokenStream {
    event_bus::subscribe(input)
}

#[proc_macro_derive(Unsubscribe)]
pub fn derive_unsubscribe(input: TokenStream) -> TokenStream {
    event_bus::unsubscribe(input)
}

#[proc_macro_derive(SpawnThread)]
pub fn derive_spawn_thread(input: TokenStream) -> TokenStream {
    spawn::spawn_thread(input)
}

#[proc_macro_derive(SpawnAsync)]
pub fn derive_spawn_async(input: TokenStream) -> TokenStream {
    spawn::spawn_async(input)
}

#[proc_macro_derive(SpawnParallel)]
pub fn derive_spawn_parallel(input: TokenStream) -> TokenStream {
    spawn::spawn_parallel(input)
}

#[proc_macro_derive(Role)]
pub fn derive_role(input: TokenStream) -> TokenStream {
    permission::role(input)
}

#[proc_macro_derive(GrantRole, attributes(syzygy))]
pub fn derive_grant_role(input: TokenStream) -> TokenStream {
    permission::grant_role(input)
}

#[proc_macro_derive(GuardRole, attributes(syzygy))]
pub fn derive_guard_role(input: TokenStream) -> TokenStream {
    permission::guard_role(input)
}

#[proc_macro_derive(ImpliedBy, attributes(syzygy))]
pub fn derive_implied_by(input: TokenStream) -> TokenStream {
    permission::implied_by(input)
}
