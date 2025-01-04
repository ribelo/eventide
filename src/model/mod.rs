use crate::context::Context;

mod unsync;

pub trait Model: Clone + Send + Sync + 'static {}
impl<T> Model for T where T: Clone + Send + Sync + 'static {}

pub trait ModelAccess<M>: Context {
    #[must_use]
    fn model(&self) -> &M;
    #[must_use]
    fn query<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(self.model())
    }
}

pub trait ModelModify<M>: ModelAccess<M> {
    #[must_use]
    fn model_mut(&mut self) -> &mut M;
    fn update<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut M) -> R,
    {
        f(self.model_mut())
    }
}

pub trait ModelSnapshot<M: Model> {
    fn snapshot(&self) -> M;
}
