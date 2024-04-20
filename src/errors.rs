#[derive(Debug, thiserror::Error)]
pub enum DispatchError<T1, T2> {
    #[error(transparent)]
    SendSystemEventError(tokio::sync::mpsc::error::SendError<T1>),
    #[error(transparent)]
    SendSideEffectError(tokio::sync::mpsc::error::SendError<T2>),
}
