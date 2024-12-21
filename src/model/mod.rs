use crate::context::Context;

mod unsync;

pub trait Model: Clone + Send + Sync + 'static {}
impl<T> Model for T where T: Clone + Send + Sync + 'static {}

pub trait ModelAccess<M>: Context {
    fn model(&self) -> &M;
    fn query<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(self.model())
    }
}

pub trait ModelModify<M>: ModelAccess<M> {
    fn model_mut(&mut self) -> &mut M;
    fn update<F>(&mut self, f: F)
    where
        F: FnOnce(&mut M),
    {
        f(self.model_mut());
    }
}
