use crate::__invoke__::{InvokeRequest, InvokeResponse};
// =============================
// Imports
// =============================
use crate::batches::{AsyncBatchWorker, AwaitSlot, BatchWorker};
use crate::structs::AppState;
use library::{Invoke, InputExt, Input, Output};
use futures::future::join_all;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use once_cell::sync::OnceCell;
use serde_json::{json, Value};
use futures::channel::oneshot;

// =============================
// Global InvokeBatcher Singleton Definition
// =============================

/// Global batching processor for invoke requests.
///
/// This singleton uses an AsyncBatchWorker that accepts batches of InvokeRequest submissions.
/// It flattens all Invokes across the submissions into one big batch, calls `contract()` on that
/// flat list to convert them into Inputs, processes those Inputs concurrently via `state.invoke()`,
/// and then reassembles the processed outputs back into the original groupings. Each group's results
/// are then converted into JSON values and wrapped in an InvokeResponse.
#[derive(Clone)]
pub struct InvokeBatcher {
    // The underlying worker implements BatchWorker with:
    //   - Task type: InvokeRequest, and
    //   - Result type: InvokeResponse.
    worker: Arc<dyn BatchWorker<InvokeRequest, InvokeResponse> + Send + Sync>,
}

// Global OnceCell to store our singleton.
static INVOKE_BATCHER: OnceCell<InvokeBatcher> = OnceCell::new();

// =============================
// Implementation of the Global InvokeBatcher
// =============================
impl InvokeBatcher {
    /// Initializes the global InvokeBatcher with the given application state.
    ///
    /// This function must be called early (e.g. at startup) before any invoke requests are handled.
    /// The processing closure here receives a batch of InvokeRequest submissions, flattens them, calls
    /// the contract method on all Invokes at once, processes the resulting Inputs concurrently via `state.invoke()`,
    /// and then deconvolutes the results back into groups corresponding to the original submissions.
    pub fn initialize(state: Arc<AppState>) {
        let worker: AsyncBatchWorker<InvokeRequest, InvokeResponse, _, _> =
            AsyncBatchWorker::new(
                move |batched_requests: Vec<InvokeRequest>| -> Pin<Box<dyn Future<Output = Vec<InvokeResponse>> + Send>> {
                    // Clone the state for use in the async processing.
                    let state_for_requests = state.clone();
                    Box::pin(async move {
                        // === FLATTENING PHASE ===
                        // We create two vectors:
                        // - `counts`: to record how many Invokes were in each submission.
                        // - `flat_invokes`: to accumulate all Invokes across the batch.
                        let mut counts = Vec::with_capacity(batched_requests.len());
                        let mut flat_invokes = Vec::new();
                        for req in batched_requests.into_iter() {
                            // Record the length of this submission.
                            counts.push(req.call.len());
                            // Extend the flat_invokes with all Invokes from this request.
                            flat_invokes.extend(req.call);
                        }
 
                        // === CONTRACT PHASE ===
                        // Instead of converting each submission separately, we now call the `contract()`
                        // method on the entire flat batch of Invokes at once.
                        // This should return a Vec<Input> in the order corresponding to flat_invokes.
                        let inputs: Vec<Input> = flat_invokes.contract();
 
                        // === PROCESSING PHASE ===
                        // Obtain a starting program counter for generating unique call IDs.
                        // Process each input concurrently via the application state's invoke method.
                        // We assign a unique call ID to each input based on the starting counter and its index.
                        let pc = crate::globals::get_pc();
                        
                        let flat_results: Vec<Option<Output>> = join_all(
                            inputs.into_iter().enumerate().map(|(i, input)| {
                                let state_clone = state_for_requests.clone();
                                let call_id = pc + i as u64;
                                async move {
                                    state_clone.invoke(call_id, input).await
                                }
                            })
                        ).await;
 
                        // === REASSEMBLY PHASE ===
                        // Now we need to split flat_results back into groups corresponding to the original submissions.
                        let mut responses = Vec::with_capacity(counts.len());
                        let mut start = 0;
                        for count in counts {
                            // Extract the slice corresponding to this submission.
                            let group: Vec<Option<Output>> = flat_results[start..start + count].to_vec();
                            // Convert each Option<Output> into a JSON Value.
                            let emit: Vec<Value> = group.into_iter()
                                .map(|opt| {
                                    serde_json::to_value(opt)
                                        .unwrap_or_else(|_| json!("serialization error"))
                                })
                                .collect();
                            responses.push(InvokeResponse { emit });
                            start += count;
                        }
                        responses
                    })
                }
            );
 
        let _ = INVOKE_BATCHER
            .set(InvokeBatcher {
                worker: Arc::new(worker),
            });
    }
 
    /// Returns a reference to the global InvokeBatcher singleton.
    pub fn get() -> &'static Self {
        INVOKE_BATCHER.get().expect("InvokeBatcher not initialized")
    }
 
    /// Submits an InvokeRequest to the global batcher and returns an AwaitSlot that resolves to an InvokeResponse.
    ///
    /// The underlying BatchWorker API expects a Vec of submissions, so we wrap the given InvokeRequest in a Vec.
    /// Then we spawn a local task that awaits the Vec response and extracts the single InvokeResponse.
    pub fn submit(&self, request: InvokeRequest) -> AwaitSlot<InvokeResponse> {
        let slot_vec = self.worker.submit(vec![request]);
        let (sender, receiver) = oneshot::channel();
 
        // Use spawn_local to await the result and extract the single response.
        tokio::task::spawn_local(async move {
            match slot_vec.get().await {
                Ok(mut responses_vec) => {
                    if let Some(response) = responses_vec.pop() {
                        let _ = sender.send(response);
                    } else {
                        eprintln!("No response received from batch processing");
                    }
                }
                Err(e) => {
                    eprintln!("Error receiving batched response: {:?}", e);
                }
            }
        });
 
        AwaitSlot(receiver)
    }
}
