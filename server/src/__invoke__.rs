use crate::structs::AppState;
use crate::invoke_batcher::InvokeBatcher;
use actix_web::{web, HttpResponse, Responder};
use library::{Invoke, Output};
use serde_json::{json, Value};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct InvokeRequest {
    pub call: Vec<Invoke>,
}

#[derive(serde::Serialize)]
pub struct InvokeResponse {
    pub emit: Vec<Value>,
}

/// The invoke handler uses the global InvokeBatcher to process an InvokeRequest.
/// It submits the full request to the batcher and awaits a single InvokeResponse.
pub async fn invoke_handler(
    _state: web::Data<AppState>,
    invoke_request: web::Json<InvokeRequest>,
) -> impl Responder {
    // Extract the InvokeRequest from the JSON payload.
    let request: InvokeRequest = invoke_request.into_inner();
    
    // Submit the InvokeRequest via the global batching processor.
    // The batching system will eventually produce a single InvokeResponse.
    let batch_result = InvokeBatcher::get().submit(request).get().await;
    
    // Return the response or an error, if one occurred.
    match batch_result {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Batch invocation failed: {:?}", e)
        })),
    }
}
