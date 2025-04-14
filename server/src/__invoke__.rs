use crate::structs::AppState;
use crate::invoke_batcher::InvokeBatcher;
use actix_web::{web, HttpResponse, Responder};
use library::{Invoke, Output};
use serde_json::{json, Value};

#[derive(serde::Deserialize)]
pub struct InvokeRequest {
    call: Vec<Invoke>,
}

#[derive(serde::Serialize)]
pub struct InvokeResponse {
    emit: Value,
}

/// The invoke handler now uses the global InvokeBatcher to submit the entire Vec<Invoke>
/// for batched processing.
pub async fn invoke_handler(
    _state: web::Data<AppState>,
    invoke_request: web::Json<InvokeRequest>,
) -> impl Responder {
    // Extract the vector of Invoke from the request.
    let invokes: Vec<Invoke> = invoke_request.into_inner().call;

    // Submit the invocation via the global batching processor.
    let batch_result = InvokeBatcher::get()
        .submit(invokes)
        .get()
        .await;

    // Map the result into our response type.
    match batch_result {
        Ok(outputs) => {
            // 'outputs' is a Vec<Option<Output>>; wrap each output in our response.
            let responses: Vec<InvokeResponse> = outputs.into_iter()
                .map(|opt| InvokeResponse { emit: json!(opt) })
                .collect();

            HttpResponse::Ok().json(responses)
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Batch invocation failed: {:?}", e)
        })),
    }
}
