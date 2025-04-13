use crate::structs::AppState;
use crate::globals::get_pc;

use actix_web::{web, HttpResponse, Responder};
use library::{InputExt, Invoke};
use serde_json::{json, Value};
use std::sync::Arc;
use futures::future::join_all;

#[derive(serde::Deserialize)]
pub struct InvokeRequest {
    call: Invoke,
}

#[derive(serde::Serialize)]
pub struct InvokeResponse {
    emit: Value,
}

pub async fn invoke_handler(
    state: web::Data<AppState>,
    invoke_requests: web::Json<Vec<InvokeRequest>>,
) -> impl Responder {
    let invokes: Vec<Invoke> = invoke_requests
        .into_inner()
        .into_iter()
        .map(|req| req.call)
        .collect();

    let contractions = invokes.contract();

    let futures = contractions.into_iter().map(|contraction| {
        let state = state.clone();
        async move {
            let result = state.invoke(get_pc(), contraction).await;
            InvokeResponse { emit: json!(result) }
        }
    });

    let results: Vec<InvokeResponse> = join_all(futures).await;

    HttpResponse::Ok().json(results)
}
