use crate::context::Context;

pub trait UnsyncModelAccess<M>: Context {
    fn model(&self) -> &M;
    fn query<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&M) -> R,
    {
        f(self.model())
    }
}

pub trait UnsyncModelModify<M>: UnsyncModelAccess<M> {
    fn model_mut(&mut self) -> &mut M;
    fn update<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut M),
    {
        f(self.model_mut());
    }
}
