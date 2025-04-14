// Import necessary items from futures, Arc, and tokio.
use futures::channel::oneshot;
use futures::Future;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

/// An awaitable slot that will eventually yield a result of type `R`.
///
/// This wraps a oneshot receiver that will complete when a result is sent.
pub struct AwaitSlot<R>(pub oneshot::Receiver<R>);

impl<R> AwaitSlot<R> {
    /// Awaits the result from the oneshot channel.
    ///
    /// Returns:
    /// - `Ok(result)` if the result is successfully received.
    /// - `Err(oneshot::Canceled)` if the sending half was dropped.
    pub async fn get(self) -> Result<R, oneshot::Canceled> {
        self.0.await
    }
}

/// A structure representing a single batch request.
///
/// Each request holds a value of type `T` (in our design, a vector of items) and
/// a responder, which is a oneshot sender used to send back the processed result.
struct Request<T, R> {
    value: T,
    responder: oneshot::Sender<R>,
}

/// A trait for batch workers which defines a synchronous `submit` function.
///
/// This trait has two generic parameters:
/// - `E`: The individual task type.
/// - `R`: The result type corresponding to each task.
/// The `submit` method accepts a vector (`Vec<E>`) of tasks and immediately returns an awaitable slot
/// that will eventually yield a vector of results (`Vec<R>`).
pub trait BatchWorker<E, R> {
    fn submit(&self, value: Vec<E>) -> AwaitSlot<Vec<R>>;
}

/// An asynchronous batch worker that collects submissions, flattens the work,
/// processes the entire batch at once, and then reassembles the results.
///
/// The worker is constructed with an asynchronous processing function (`process_fn`)
/// which takes a flattened `Vec<E>`—representing the concatenation of all individual tasks
/// (submissions are combined)—and returns a Future that resolves to a `Vec<R>`.
///
/// The type parameters are:
/// - `E`: The individual task type (e.g. `Invoke`).
/// - `R`: The result type for a single task.
/// - `F`: The function type for the processing function.
/// - `Fut`: The future type returned by the processing function.
pub struct AsyncBatchWorker<E, R, F, Fut>
where
    // The processing function must be thread-safe (Send + Sync) and 'static.
    F: Fn(Vec<E>) -> Fut + Send + Sync + 'static,
    // The processing future must be Send and return a Vec<R>.
    Fut: Future<Output = Vec<R>> + Send + 'static,
    // E and R must be Send and 'static.
    E: Send + 'static,
    R: Send + 'static,
{
    // Shared processing function.
    process_fn: Arc<F>,
    // Channel to receive individual batch requests.
    tx: UnboundedSender<Request<Vec<E>, Vec<R>>>,
}

impl<E, R, F, Fut> AsyncBatchWorker<E, R, F, Fut>
where
    F: Fn(Vec<E>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<R>> + Send + 'static,
    E: Send + 'static,
    R: Send + 'static,
{
    /// Creates a new AsyncBatchWorker given the processing function.
    ///
    /// It creates an unbounded MPSC channel to receive incoming submissions. It then spawns
    /// a background task (via Tokio's spawn) to process the requests in batch.
    pub fn new(process_fn: F) -> Self {
        // Create an unbounded channel to send/receive requests.
        let (tx, rx) = mpsc::unbounded_channel();
        let worker = AsyncBatchWorker {
            process_fn: Arc::new(process_fn),
            tx,
        };
        // Start the background batching process.
        worker.start(rx);
        worker
    }

    /// Starts the background task that continuously receives and processes requests.
    ///
    /// The logic is as follows:
    /// 1. Wait until at least one request is received.
    /// 2. Collect as many pending requests as possible (using try_recv).
    /// 3. For each request, record its number of items and accumulate all items into a single flattened vector.
    /// 4. Process the flattened vector with the provided async function.
    /// 5. Reassemble the flat results back into the original submission groups and send each group back.
    fn start(&self, mut rx: UnboundedReceiver<Request<Vec<E>, Vec<R>>>) {
        // Clone the processing function for the async task.
        let process_fn = Arc::clone(&self.process_fn);
        tokio::spawn(async move {
            loop {
                // Wait for at least one submission.
                let first = match rx.recv().await {
                    Some(req) => req,
                    None => break, // Channel closed; no more submissions.
                };

                // Begin a batch with the first received request.
                let mut batch = vec![first];

                // Non-blockingly drain additional requests available on the channel.
                while let Ok(req) = rx.try_recv() {
                    batch.push(req);
                }

                // Initialize vectors to store:
                // - `counts`: the length of each individual submission (number of tasks).
                // - `responders`: the oneshot channels to send back each submission's results.
                // - `flattened`: a flattened vector containing all tasks from all submissions.
                let mut counts = Vec::with_capacity(batch.len());
                let mut responders = Vec::with_capacity(batch.len());
                let mut flattened = Vec::new();
                for req in batch {
                    counts.push(req.value.len());
                    responders.push(req.responder);
                    flattened.extend(req.value);
                }

                // Process the entire flattened batch using the provided asynchronous function.
                let mut flat_results = (process_fn)(flattened).await;

                // Reassemble the results by using the recorded counts.
                // We use `drain(0..count)` to move items without cloning.
                for (count, responder) in counts.into_iter().zip(responders.into_iter()) {
                    let segment: Vec<R> = flat_results.drain(0..count).collect();
                    // Send the segmented results back via the oneshot channel.
                    let _ = responder.send(segment);
                }
            }
        });
    }
}

impl<E, R, F, Fut> BatchWorker<E, R> for AsyncBatchWorker<E, R, F, Fut>
where
    F: Fn(Vec<E>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Vec<R>> + Send + 'static,
    E: Send + 'static,
    R: Send + 'static,
{
    /// Submits a vector of tasks for processing and returns an awaitable slot.
    /// The slot will eventually yield a vector of results corresponding to the input tasks.
    fn submit(&self, value: Vec<E>) -> AwaitSlot<Vec<R>> {
        // Create a new oneshot channel for the response.
        let (sender, receiver) = oneshot::channel();
        // Wrap the submission and the sender into a Request.
        let req = Request { value, responder: sender };
        // Send the request through the channel. In a production setting you might
        // handle the error when the receiver has been dropped.
        self.tx.send(req).expect("Worker has shut down");
        AwaitSlot(receiver)
    }
}
