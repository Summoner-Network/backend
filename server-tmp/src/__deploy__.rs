use crate::structs::{AppState, ContractSetArg};

use actix_web::{web, HttpResponse, Responder};
use serde_json::{json};

/// Deploy contract endpoint. Computes the contract’s hash and upserts the contract code.
pub async fn deploy_contract(
    state: web::Data<AppState>,
    body: web::Bytes,
) -> impl Responder {
    if body.is_empty() {
        return HttpResponse::BadRequest()
            .json(json!({"error": "No contract bytes provided"}));
    }

    // Compute contract hash.
    let contract_hash = blake3::hash(&body);
    
    // Upsert the contract into the contracts table.
    let set_arg = ContractSetArg {
        is_contract: true,
        in_contract: contract_hash.as_bytes().to_vec(),
        at_address: contract_hash.as_bytes().to_vec(),
        data: body.into(),
        version: None,
    };
    let result = state.set_contracts(vec![set_arg]).await;
    if let Err(e) = result {
        return HttpResponse::InternalServerError().json(json!({"error": format!("Deployment failed: {}", e)}));
    }

    HttpResponse::Ok().json(json!({
        "status": "Contract deployed",
        "contract_hash": hex::encode(contract_hash.as_bytes())
    }))
}