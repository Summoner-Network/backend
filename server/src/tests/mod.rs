use actix_web::{
    test, web, App,
    dev::{ServiceFactory, ServiceResponse, ServiceRequest},
    body::MessageBody,
};
use std::fmt::Debug;
use serde_json::json;
use library::Input;
use crate::{
    AppState,
    create_linker,
    deploy_contract,
    invoke_handler,
    __invoke__::InvokeRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, Error as AnyError};

// Add the GENESIS_SQL constant
static GENESIS_SQL: &str = include_str!("../../sql/genesis.sql");

async fn setup_database(pool: &deadpool_postgres::Pool) -> Result<()> {
    let client = pool.get().await
        .map_err(|e| AnyError::msg(format!("Failed to get client: {}", e)))?;
    
    // First truncate all tables
    client.batch_execute(
        "TRUNCATE TABLE chunks, blocks, settles, contracts CASCADE;"
    ).await
        .map_err(|e| AnyError::msg(format!("Failed to truncate tables: {}", e)))?;

    Ok(())
}

async fn setup_test_app() -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = actix_web::Error,
        InitError = (),
    >
> {
    // Create test database pool with localhost instead of service name
    let mut cfg = deadpool_postgres::Config::new();
    cfg.dbname = Some("mydb_test".into());
    cfg.user = Some("postgres".into());
    cfg.password = Some("password".into());
    cfg.host = Some("localhost".into());
    cfg.port = Some(5433);
    cfg.connect_timeout = Some(std::time::Duration::from_secs(5));
    
    let pool = cfg.create_pool(Some(deadpool_postgres::Runtime::Tokio1), tokio_postgres::NoTls)
        .expect("Failed to create test pool");

    // Setup database schema using genesis SQL if tables don't exist
    let client = pool.get().await.expect("Failed to get client");
    
    // Check if tables exist, if not create them
    let tables_exist = client.query_one(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_name = 'contracts'
        )", &[]
    ).await.expect("Failed to check tables")
        .get::<_, bool>(0);

    if !tables_exist {
        client.batch_execute(GENESIS_SQL)
            .await
            .expect("Failed to execute genesis SQL");
    }

    // Truncate all tables for clean state
    if let Err(e) = setup_database(&pool).await {
        eprintln!("Failed to setup database: {}", e);
        // Continue anyway - the tests will handle connection failures
    }

    // Initialize test state
    let mut state = AppState {
        db_pool: pool.clone(),
        linker: None,
    };

    let linker = create_linker(&state).expect("Failed to create test linker");
    state.linker = Some(linker);

    // Create test app
    App::new()
        .app_data(web::Data::new(state))
        .route("/deploy", web::post().to(deploy_contract))
        .route("/invoke", web::post().to(invoke_handler))
}

#[actix_web::test]
async fn test_deploy_and_invoke_workflow() {
    let app = test::init_service(setup_test_app().await).await;

    let wasm_bytes = mock::create_test_wasm();
    let deploy_req = test::TestRequest::post()
        .uri("/deploy")
        .set_payload(wasm_bytes.clone())
        .insert_header(("content-type", "application/octet-stream"))
        .to_request();

    let deploy_resp = test::call_service(&app, deploy_req).await;
    let status = deploy_resp.status();
    let body = test::read_body(deploy_resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    
    if !status.is_success() {
        eprintln!("Deploy response: {}", body_str);
        if body_str.contains("connection") {
            eprintln!("Skipping test due to database connection issues");
            return;
        }
    }
    
    assert!(status.is_success(), "Deploy failed with status: {}, body: {}", status, body_str);
    
    let deploy_body: serde_json::Value = serde_json::from_str(&body_str)
        .expect("Failed to parse deploy response as JSON");
    
    let contract_hash = deploy_body.get("contract_hash")
        .and_then(|v| v.as_str())
        .expect("Expected contract_hash in deploy response");

    let batch_input = vec![Input {
        contract: hex::decode(contract_hash).unwrap(),
        functions: {
            let mut m = HashMap::new();
            m.insert("enter_contract".to_string(), vec![json!({})]);
            m
        }
    }];

    let invoke_req = test::TestRequest::post()
        .uri("/invoke")
        .set_json(&batch_input)
        .to_request();

    let invoke_resp = test::call_service(&app, invoke_req).await;
    let status = invoke_resp.status();
    let body = test::read_body(invoke_resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    assert!(status.is_success(), "Invoke failed with status: {}, body: {}", status, body_str);
}

#[actix_web::test]
async fn test_deploy_duplicate_contract() {
    let app = test::init_service(setup_test_app().await).await;
    let wasm_bytes = mock::create_test_wasm();

    let deploy_1 = test::TestRequest::post()
        .uri("/deploy")
        .set_payload(wasm_bytes.clone())
        .insert_header(("content-type", "application/octet-stream"))
        .to_request();

    let deploy_resp1 = test::call_service(&app, deploy_1).await;
    let status = deploy_resp1.status();
    let body = test::read_body(deploy_resp1).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    
    if !status.is_success() {
        if body_str.contains("connection") {
            eprintln!("Skipping test due to database connection issues");
            return;
        }
    }
    
    assert!(status.is_success(), "First deploy failed: {}", body_str);
    let body1: serde_json::Value = serde_json::from_str(&body_str)
        .expect("Failed to parse first deploy response as JSON");

    let deploy_2 = test::TestRequest::post()
        .uri("/deploy")
        .set_payload(wasm_bytes)
        .insert_header(("content-type", "application/octet-stream"))
        .to_request();

    let deploy_resp2 = test::call_service(&app, deploy_2).await;
    let status = deploy_resp2.status();
    let body = test::read_body(deploy_resp2).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    
    assert!(status.is_success(), "Second deploy failed: {}", body_str);
    let body2: serde_json::Value = serde_json::from_str(&body_str)
        .expect("Failed to parse second deploy response as JSON");

    assert_eq!(body1["contract_hash"], body2["contract_hash"]);
}

#[actix_web::test]
async fn test_invoke_invalid_function() {
    let app = test::init_service(setup_test_app().await).await;

    let wasm_bytes = mock::create_test_wasm();
    let deploy_req = test::TestRequest::post()
        .uri("/deploy")
        .set_payload(wasm_bytes)
        .insert_header(("content-type", "application/octet-stream"))
        .to_request();

    let deploy_resp = test::call_service(&app, deploy_req).await;
    let status = deploy_resp.status();
    let body = test::read_body(deploy_resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    
    if !status.is_success() {
        if body_str.contains("connection") {
            eprintln!("Skipping test due to database connection issues");
            return;
        }
    }
    
    assert!(status.is_success(), "Deploy failed: {}", body_str);
    let deploy_body: serde_json::Value = serde_json::from_str(&body_str)
        .expect("Failed to parse deploy response as JSON");
    
    let contract_hash = deploy_body["contract_hash"].as_str()
        .expect("Expected contract_hash in deploy response");

    let batch_input = vec![Input {
        contract: hex::decode(contract_hash).unwrap(),
        functions: {
            let mut m = HashMap::new();
            m.insert("non_existent_function".to_string(), vec![json!({})]);
            m
        }
    }];

    let invoke_req = test::TestRequest::post()
        .uri("/invoke")
        .set_json(&batch_input)
        .to_request();

    let resp = test::call_service(&app, invoke_req).await;
    assert!(resp.status().is_client_error());
}

#[actix_web::test]
async fn test_deploy_invalid_wasm() {
    let app = test::init_service(setup_test_app().await).await;
    
    // Create completely invalid data - not even close to WASM format
    let invalid_wasm = b"This is definitely not a WASM binary file!!!!".to_vec();

    let req = test::TestRequest::post()
        .uri("/deploy")
        .set_payload(invalid_wasm)
        .insert_header(("content-type", "application/octet-stream"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    let body_str = String::from_utf8(body.to_vec()).unwrap_or_default();
    
    eprintln!("Deploy invalid WASM - Response status: {}, body: {}", status, body_str);
    
    // Check both status code and error message
    assert!(
        status.is_client_error() && body_str.contains("error"),
        "Expected client error status and error message, got status {} with body: {}", 
        status, 
        body_str
    );
}

#[actix_web::test]
async fn test_invoke_nonexistent_contract() {
    let app = test::init_service(setup_test_app().await).await;

    let batch_input = vec![Input {
        contract: hex::decode("deadbeef").unwrap(),
        functions: {
            let mut m = HashMap::new();
            m.insert("some_function".to_string(), vec![json!({})]);
            m
        }
    }];

    let req = test::TestRequest::post()
        .uri("/invoke")
        .set_json(&batch_input)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_client_error());
}

mod mock {
    pub fn create_test_wasm() -> Vec<u8> {
        // Create a minimal valid WebAssembly module
        vec![
            // Magic module header (0x6d736100 in little endian)
            0x00, 0x61, 0x73, 0x6D,
            // Version (1)
            0x01, 0x00, 0x00, 0x00,

            // Type section (1)
            0x01, 0x04, 0x01,           // section code, section size, num types
            0x60, 0x00, 0x00,           // type 0: () -> ()

            // Function section (3)
            0x03, 0x02, 0x01, 0x00,     // section code, size, num funcs, type idx

            // Memory section (5)
            0x05, 0x03, 0x01,           // section code, section size, num memories
            0x00, 0x01,                 // memory 0: limits: flags=0, initial=1

            // Export section (7)
            0x07, 0x17, 0x02,           // section code, section size, num exports
            // Export 0: "memory"
            0x06, 0x6D, 0x65, 0x6D, 0x6F, 0x72, 0x79, // name: "memory"
            0x02, 0x00,                 // export kind=memory, index=0
            // Export 1: "enter_contract"
            0x0D, 0x65, 0x6E, 0x74, 0x65, 0x72, 0x5F, 0x63, 0x6F, 0x6E, 0x74, 0x72, 0x61, 0x63, 0x74, // name: "enter_contract"
            0x00, 0x00,                 // export kind=function, index=0

            // Code section (10)
            0x0A, 0x04, 0x01,           // section code, section size, num functions
            0x02, 0x00,                 // func 0 body size, local decl count
            0x0B,                       // end
        ]
    }
}
