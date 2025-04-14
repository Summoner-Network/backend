mod structs;
mod __invoke__;
mod __deploy__;
mod bytemath;

use anyhow::{Result, anyhow};
use std::{sync::Arc, thread, time::Duration};

use __invoke__::invoke_handler;
use __deploy__::deploy_contract;
use structs::{AppState, ContractContext, ContractSetArg};
use actix_web::{web::{self, Data}, App, HttpResponse, HttpServer, Responder};
use pqcrypto_dilithium::dilithium2;
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use wasmtime::{Caller, Engine, Linker, Memory};
use rand::RngCore;
use wasmtime::Config as WASMConfig;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Wait for dependent services.
    thread::sleep(Duration::from_secs(20));
    unsafe {
        std::env::set_var("RUST_LOG", "debug");
    }
    env_logger::init();
    
    // Set up keys and verify a signature.
    let (public_key, secret_key) = dilithium2::keypair();
    let message = b"Hello, Dilithium!";
    let signature = dilithium2::sign(message, &secret_key);
    let is_valid = dilithium2::open(&signature, &public_key)
        .map(|opened_message| opened_message == message)
        .unwrap_or(false);
    println!(
        "{}",
        if is_valid {
            "Signature verified successfully!"
        } else {
            "Signature verification failed."
        }
    );

    // Set up PostgreSQL connection pool.
    let mut cfg = Config::new();
    cfg.dbname = Some("mydb".into());
    cfg.user = Some("postgres".into());
    cfg.password = Some("password".into());
    cfg.host = Some("postgres".into());
    cfg.port = Some(5432);
    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });
    cfg.pool = Some(deadpool_postgres::PoolConfig::new(2 << 16));

    let pool = cfg
        .create_pool(Some(deadpool_postgres::Runtime::Tokio1), NoTls)
        .expect("Failed to create PostgreSQL pool");

    // Execute genesis SQL.
    static GENESIS_SQL: &str = include_str!("../sql/genesis.sql");
    {
        let client = pool.get().await.expect("Failed to get client from pool");
        client
            .batch_execute(GENESIS_SQL)
            .await
            .expect("Failed to execute genesis SQL");
        println!("Genesis SQL executed successfully.");
    }

    println!("Hello, world!");

    // Initialize Wasmtime engine and application state.
    let mut state = AppState {
        db_pool: pool.clone(),
        linker: None,
    };

    let linker = create_linker(&state);
    state.linker = Some(linker.expect("Linker error!"));

    // Run the HTTP server with the /invoke endpoint.
    HttpServer::new(move || {
        App::new()
            // If needed, share the pool with your handlers:
            .app_data(Data::new(state.clone()))
            .route("/deploy", web::post().to(deploy_contract))
            .route("/invoke", web::post().to(invoke_handler))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

/// Unlike the fixed 64‐byte approach, here we assume the guest lumps all addresses in a JSON array
/// passed in `keys_ptr, keys_len`. We parse them as `Vec<(Vec<u8>, Vec<u8>)>` (or single addresses,
/// depending on your usage). This way, we do not assume any specific size for each address.
/// We do similarly for sets.

pub fn create_linker(state: &AppState) -> Result<Linker<ContractContext>> {
    let engine = Engine::default();
    let mut linker = Linker::new(&engine);

    // -----------------------------------------------------------------------------
    // env.rand
    // -----------------------------------------------------------------------------
    // Provide 32 random bytes to the caller.
    linker.func_wrap("env", "rand", move |mut caller: Caller<'_, ContractContext>, out_ptr: u32, out_len_ptr: u32| {
        let memory = caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or_else(|| anyhow!("Failed to find `memory` export"))?;

        // Generate 32 bytes of randomness.
        let mut rand_buf = [0u8; 32];
        rand::rng().fill_bytes(&mut rand_buf);

        // Write them into guest memory.
        memory.write(&mut caller, out_ptr as usize, &rand_buf)
            .map_err(|e| anyhow!("Error writing rand to guest memory: {}", e))?;

        // Write the length (32) as a little-endian u32.
        let len_bytes = (rand_buf.len() as u32).to_le_bytes();
        memory.write(&mut caller, out_len_ptr as usize, &len_bytes)
            .map_err(|e| anyhow!("Error writing rand length: {}", e))?;

        Ok(0)
    })?;

    // -----------------------------------------------------------------------------
    // env.gets
    // -----------------------------------------------------------------------------
    // Signature: fn gets(keys_ptr: (u32,u32), rets_ptr: (u32,u32)) -> i32
    // Instead of a fixed chunk approach, we parse keys_ptr as a JSON array.
    // For instance, if the guest wants to fetch multiple addresses, it will JSON‐serialize them
    // as `[[in_contract1, at_address1], [in_contract2, at_address2], ...]`.
    // Then we return the DB results in a single JSON array.

    let get_state = state.clone();
    linker.func_wrap("env", "gets", move |
        mut caller: Caller<'_, ContractContext>,
        keys_ptr: u32,
        keys_len: u32,
        rets_ptr: u32,
        rets_len_ptr: u32|
    {
        let memory = caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or_else(|| anyhow!("Failed to find `memory` export"))?;

        // 1) Read JSON data for the key list.
        let mut keys_buf = vec![0u8; keys_len as usize];
        memory.read(&caller, keys_ptr as usize, &mut keys_buf)
            .map_err(|e| anyhow!("Error reading keys buffer: {}", e))?;

        // 2) Parse the JSON array of `(Vec<u8>, Vec<u8>)` pairs.
        // If you only store a single address, you could parse as `Vec<Vec<u8>>`, etc.
        let pairs: Vec<(Vec<u8>, Vec<u8>)> = match serde_json::from_slice(&keys_buf) {
            Ok(p) => p,
            Err(e) => {
                // If we cannot parse, we can return 0-length output.
                eprintln!("gets: JSON parse error: {}", e);
                Vec::new()
            }
        };

        // 3) Fetch all records.
        let records = futures::executor::block_on(get_state.get_contracts(pairs))
            .map_err(|e| anyhow!("DB get_contracts failed: {}", e))?;

        // 4) Convert to JSON.
        let out_json = serde_json::to_vec(&records)
            .map_err(|e| anyhow!("Cannot serialize records to JSON: {}", e))?;

        // 5) Write the resulting bytes into guest memory.
        memory.write(&mut caller, rets_ptr as usize, &out_json)
            .map_err(|e| anyhow!("Error writing gets result: {}", e))?;
        // Then store the length at rets_len_ptr.
        let len_bytes = (out_json.len() as u32).to_le_bytes();
        memory.write(&mut caller, rets_len_ptr as usize, &len_bytes)
            .map_err(|e| anyhow!("Error writing gets length: {}", e))?;

        Ok(0)
    })?;

    // -----------------------------------------------------------------------------
    // env.sets
    // -----------------------------------------------------------------------------
    // Similarly, we parse the keys as JSON array (if needed), parse the vals as JSON array,
    // combine them, and pass them to AppState.

    let set_state = state.clone();
    linker.func_wrap("env", "sets", move |
        mut caller: Caller<'_, ContractContext>,
        keys_ptr: u32,
        keys_len: u32,
        vals_ptr: u32,
        vals_len: u32|
    {
        let memory = caller
            .get_export("memory")
            .and_then(|e| e.into_memory())
            .ok_or_else(|| anyhow!("Failed to find `memory` export"))?;

        // 1) Read keys JSON.
        let mut keys_buf = vec![0u8; keys_len as usize];
        memory.read(&caller, keys_ptr as usize, &mut keys_buf)
            .map_err(|e| anyhow!("sets: error reading keys buffer: {}", e))?;
        let pairs: Vec<(Vec<u8>, Vec<u8>)> = match serde_json::from_slice(&keys_buf) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("sets: JSON parse error in keys: {}", e);
                Vec::new()
            }
        };

        // 2) Read vals JSON.
        let mut vals_buf = vec![0u8; vals_len as usize];
        memory.read(&caller, vals_ptr as usize, &mut vals_buf)
            .map_err(|e| anyhow!("sets: error reading vals buffer: {}", e))?;
        let parsed_args: Vec<ContractSetArg> = match serde_json::from_slice(&vals_buf) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("sets: JSON parse error in vals: {}", e);
                Vec::new()
            }
        };

        // We assume the user either includes in_contract/at_address in the JSON, or we want to override them.
        // If we want to override with the pairs, we can do so.
        if parsed_args.len() != pairs.len() {
            eprintln!("Warning: # of pairs ({}) != # of ContractSetArg items ({})", pairs.len(), parsed_args.len());
        }

        // We'll build the final set of ContractSetArg.
        let mut merged = Vec::new();
        let count = pairs.len().max(parsed_args.len());
        for i in 0..count {
            // If we have a pair, use that.
            let (some_in, some_at) = if i < pairs.len() {
                pairs[i].clone()
            } else {
                (vec![], vec![])
            };

            if i < parsed_args.len() {
                let mut arg = parsed_args[i].clone();
                // override.
                arg.in_contract = some_in;
                arg.at_address = some_at;
                merged.push(arg);
            } else {
                // no data => create empty.
                merged.push(ContractSetArg {
                    is_contract: false,
                    in_contract: some_in,
                    at_address: some_at,
                    data: vec![],
                    version: None,
                });
            }
        }

        // 3) Do the DB update.
        futures::executor::block_on(set_state.set_contracts(merged))
            .map_err(|e| anyhow!("set_contracts failed: {}", e))?;

        // 4) Return 0.
        Ok(0)
    })?;

    Ok(linker)
}

#[cfg(test)]
mod tests;