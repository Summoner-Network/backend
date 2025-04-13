use crate::structs::AppState;
use crate::globals::get_pc;

use actix_web::{web, HttpResponse, Responder};
use library::{InputExt, Invoke};
use serde_json::{json, Value};
use std::sync::Arc;


#[derive(serde::Deserialize)]
pub struct InvokeRequest {
    call: Invoke
}

#[derive(serde::Serialize)]
pub struct InvokeResponse {
    emit: Value
}

pub async fn invoke_handler(
    state: web::Data<AppState>,
    invoke_requests: web::Json<Vec<InvokeRequest>>
) -> impl Responder {
    // Process each InvokeRequest and produce a corresponding InvokeResponse.
    // For now, this example simply creates a placeholder response for each request.
    let invokes: Vec<Invoke> = invoke_requests
        .into_inner()
        .into_iter()
        .map(|_req| {
            _req.call
        })
        .collect();

    let contractions = invokes.contract();
    let mut results = vec![];
    for contraction in contractions {
        let response = InvokeResponse { emit: json!(state.invoke(get_pc(), contraction).await) };
        results.push(response);
    }

    HttpResponse::Ok().json(results)
}