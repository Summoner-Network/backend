// batch_worker.rs

use futures::channel::oneshot;
use futures::Future;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// An awaitable slot that will eventually yield a result of type `R`.
pub struct AwaitSlot<R>(oneshot::Receiver<R>);

impl<R> AwaitSlot<R> {
    /// Awaits the result.
    pub async fn get(self) -> Result<R, oneshot::Canceled> {
        self.0.await
    }
}

/// A request that contains a value and a oneshot responder to send back the result.
struct Request<T, R> {
    value: T,
    responder: oneshot::Sender<R>,
}

/// A trait for batch workers with a synchronous `submit` method.
pub trait BatchWorker<T, R> {
    /// Submits a value for processing and immediately returns an awaitable slot.
    fn submit(&self, value: T) -> AwaitSlot<R>;
}

/// An asynchronous batch worker that processes one batch at a time as soon as tasks are available.
/// 
/// The worker is constructed with an asynchronous function (`process_fn`) that takes a batch (`Vec<T>`)
/// and returns a future resolving to a `Vec<R>`. It is assumed that the order of the results matches
/// the order of the submitted values.
pub struct AsyncBatchWorker<T, R, F, Fut>
where
    F: Fn(Vec<T>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<R>> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    process_fn: Arc<F>,
    tx: UnboundedSender<Request<T, R>>,
}

impl<T, R, F, Fut> AsyncBatchWorker<T, R, F, Fut>
where
    F: Fn(Vec<T>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<R>> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    /// Creates a new `AsyncBatchWorker` with the provided asynchronous batch processing function.
    pub fn new(process_fn: F) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let worker = AsyncBatchWorker {
            process_fn: Arc::new(process_fn),
            tx,
        };
        worker.start(rx);
        worker
    }

    /// Starts the background task that continuously receives requests and processes them in batches.
    fn start(&self, mut rx: UnboundedReceiver<Request<T, R>>) {
        let process_fn = Arc::clone(&self.process_fn);
        tokio::spawn(async move {
            loop {
                // Wait for at least one request.
                let first = match rx.recv().await {
                    Some(req) => req,
                    None => break, // All senders have been dropped.
                };

                // Begin a new batch with the first request.
                let mut batch = vec![first];

                // Drain any additional requests that are immediately available.
                while let Ok(req) = rx.try_recv() {
                    batch.push(req);
                }

                // Separate the submitted values and responders.
                let (values, responders): (Vec<T>, Vec<oneshot::Sender<R>>) =
                    batch.into_iter().map(|req| (req.value, req.responder)).unzip();

                // Process the batch using the provided asynchronous function.
                let results = (process_fn)(values).await;

                // Send each result back to the corresponding requestor.
                for (responder, result) in responders.into_iter().zip(results.into_iter()) {
                    let _ = responder.send(result);
                }
            }
        });
    }
}

impl<T, R, F, Fut> BatchWorker<T, R> for AsyncBatchWorker<T, R, F, Fut>
where
    F: Fn(Vec<T>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<R>> + Send + 'static,
    T: Send + 'static,
    R: Send + 'static,
{
    fn submit(&self, value: T) -> AwaitSlot<R> {
        let (sender, receiver) = oneshot::channel();
        let req = Request { value, responder: sender };
        // In a production system, you might handle errors if the worker is shut down.
        self.tx.send(req).expect("Worker has shut down");
        AwaitSlot(receiver)
    }
}
