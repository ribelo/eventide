pub trait Role: Clone + Copy + 'static {}

pub trait RoleHolder {
    type Role: Role;
}

pub trait RoleGuarded {
    type Role: Role;
}

pub trait ImpliedBy<P: Role> {}

#[derive(Clone, Copy, Default)]
pub struct Root;

impl Role for Root {}

impl<P: Role> ImpliedBy<Root> for P {}

#[derive(Clone, Copy, Default)]
pub struct AnyRole;

impl Role for AnyRole {}
