pub trait Permission: Clone + Copy + Default + 'static {}

pub trait PermissionHolder {
    type Granted: Permission;
}

pub trait PermissionGuarded {
    type Needed: Permission;
}

pub trait ImpliedBy<P: Permission> {}

#[derive(Clone, Copy, Default)]
pub struct Root;

impl Permission for Root {}

#[derive(Clone, Copy, Default)]
pub struct None;

impl Permission for None {}

impl<P: Permission> ImpliedBy<Root> for P {}
