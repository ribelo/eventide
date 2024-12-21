use crate::context::Context;

pub trait UnsyncModelAccess<M>: Context {
    fn model(&self) -> &M;
    fn query<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(&*self.model())
    }
}

pub trait UnsyncModelModify<M>: UnsyncModelAccess<M> {
    fn model_mut(&mut self) -> &mut M;
    fn update<F>(&mut self, f: F)
    where
        F: FnOnce(&mut M),
    {
        f(&mut *self.model_mut());
    }
}
