use std::fmt;

use downcast_rs::DowncastSync;
use dyn_clone::DynClone;

pub mod errors;
pub mod eve;
pub mod event;
pub mod id;
pub mod reactive;
pub mod subscription;

pub trait BoxableValue: DowncastSync + DynClone + fmt::Debug {}
dyn_clone::clone_trait_object!(BoxableValue);
downcast_rs::impl_downcast!(sync BoxableValue);
impl<T: Send + Sync + Clone + fmt::Debug + 'static> BoxableValue for T {}
