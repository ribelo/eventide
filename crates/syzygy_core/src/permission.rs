pub trait Permission: Clone + Copy + Default + 'static {}

pub trait HasPermission {
    type Permission: Permission;
}

pub trait RequiredPermission {
    type Required: Permission;
}

pub trait ImpliedBy<P: Permission> {}

#[derive(Clone, Copy, Default)]
pub struct Root;

impl Permission for Root {}

#[derive(Clone, Copy, Default)]
pub struct None;

impl Permission for None {}

impl<P: Permission> ImpliedBy<Root> for P {}
