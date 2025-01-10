use crate::context::Context;

mod unsync;

pub trait Model: Send + Sync + 'static {
    type Snapshot: Clone + Send + Sync + 'static;
    fn into_snapshot(&self) -> Self::Snapshot;
}

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

pub trait ModelSnapshotAccess: Context {
    #[must_use]
    fn snapshot(&self) -> &<<Self as Context>::Model as Model>::Snapshot;
    #[must_use]
    fn query_snapshot<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&<<Self as Context>::Model as Model>::Snapshot) -> R,
    {
        f(self.snapshot())
    }
}

pub trait ModelSnapshotCreate: Context {
    #[must_use]
    fn create_snapshot(&self) -> <<Self as Context>::Model as Model>::Snapshot;
}
