use syzygy_core::{prelude::*, syzygy::Syzygy};
use syzygy_derive::*;

#[derive(Clone, Copy, Role)]
pub struct SomePermission;

#[derive(Clone, Copy, Role, ImpliedBy)]
#[syzygy(implied_by = SomePermission)]
pub struct OtherPermission;


#[derive(Clone, Copy, Role, ImpliedBy)]
#[syzygy(implied_by = SomePermission)]
#[syzygy(implied_by = OtherPermission)]
pub struct YetAnotherPermission;

#[derive(Clone, Context, GrantRole)]
#[syzygy(role = SomePermission)]
pub struct SomeContext;

#[derive(Clone, GuardRole)]
#[syzygy(role = SomePermission)]
pub struct SomeGuarded;
