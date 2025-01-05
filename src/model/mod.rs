use crate::context::Context;

mod unsync;

pub trait Model: Clone + Send + Sync + 'static {}
impl<T> Model for T where T: Clone + Send + Sync + 'static {}

pub trait ModelAccess: Context {
    #[must_use]
    fn model(&self) -> &Self::Model;
    #[must_use]
    fn query<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Self::Model) -> R,
    {
        f(self.model())
    }
}

pub trait ModelModify: ModelAccess {
    #[must_use]
    fn model_mut(&mut self) -> &mut Self::Model;
    fn update<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self::Model) -> R,
    {
        f(self.model_mut())
    }
}

pub trait ModelSnapshot<M: Model> {
    fn snapshot(&self) -> M;
}
