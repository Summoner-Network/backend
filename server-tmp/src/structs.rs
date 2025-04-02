use std::collections::HashMap;

use deadpool_postgres::{Client, Pool};
use library::{bytea, Input, Output};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_postgres::types::ToSql;
use wasmtime::{Linker, Module, Store};

type Hashing = [u8];

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Pool,
    pub linker: Option<Linker<ContractContext>>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractContext {
    contract_id: bytea,
    call_id: u64,
    rand_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContractRecord {
    pub is_contract: bool,
    pub in_contract: Vec<u8>,
    pub at_address: Vec<u8>,
    pub data: Vec<u8>,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractSetArg {
    pub is_contract: bool,
    pub in_contract: Vec<u8>,
    pub at_address: Vec<u8>,
    pub data: Vec<u8>,
    pub version: Option<i64>, // Optional version for conditional upsert
}

impl AppState {
    pub async fn invoke(&self, call_id: u64, argument: Input) -> Option<Output> {
        //
        // 1) Load the code from DB.
        //
        let record = match self
            .get_contracts(vec![(argument.contract.clone(), argument.contract.clone())])
            .await
        {
            Ok(mut rows) => {
                if rows.is_empty() {
                    return None;
                }
                rows.remove(0)
            }
            Err(e) => {
                eprintln!("Error fetching contract: {}", e);
                return None;
            }
        };

        let contract_data = match record {
            Some(rec) if rec.is_contract => rec.data,
            _ => {
                // Either no record found, or is_contract == false.
                return None;
            }
        };

        //
        // 2) Use our existing Linker to instantiate the module.
        //
        let linker = match &self.linker {
            Some(l) => l,
            None => {
                eprintln!("No linker available in AppState.");
                return None;
            }
        };
        let engine = linker.engine();

        let module = match Module::new(engine, &contract_data) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to compile Wasm module: {}", e);
                return None;
            }
        };

        // Create the Wasmtime `Store` with a `ContractContext`. You can fill in call_id/rand_id
        // as needed for your use case. 
        let ctx = ContractContext {
            contract_id: argument.contract.clone(),
            call_id,
            rand_id: 0,
        };
        let mut store = Store::new(engine, ctx);

        let instance = match linker.instantiate(&mut store, &module) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Failed to instantiate contract: {}", e);
                return None;
            }
        };

        // Fetch the contract’s memory export so we can pass/receive data.
        let memory = match instance.get_memory(&mut store, "memory") {
            Some(mem) => mem,
            None => {
                eprintln!("No exported memory named 'memory' in contract.");
                return None;
            }
        };

        //
        // 3) Prepare the input bytes (e.g. JSON) and write them into guest memory.
        //
        //    In this example, we assume the contract’s `_invoke` function wants (in_ptr, in_len) 
        //    and will also write its output into a known region of memory. Adjust as needed.
        //
        let input_bytes = match serde_json::to_vec(&argument) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Failed to serialize Input: {}", e);
                return None;
            }
        };

        // Pick some offset in guest memory for the input, e.g. 0.
        // In a production environment, you might want to manage memory addresses more robustly.
        let in_ptr = 0;
        if let Err(e) = memory.write(&mut store, in_ptr, &input_bytes) {
            eprintln!("Failed writing input bytes to guest memory: {}", e);
            return None;
        }

        //
        // 4) Call the contract’s `_invoke` function.
        //    Suppose `_invoke` has the signature:  fn _invoke(in_ptr: i32, in_len: i32) -> i32 
        //    returning the output length.  (Adjust to fit your real contract code!)
        //
        let invoke_func = match instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "_invoke")
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("No function '_invoke(in_ptr,in_len)->i32' in Wasm: {}", e);
                return None;
            }
        };

        // Here, call `_invoke` with (in_ptr, in_len).
        let output_len_i32 = match invoke_func.call(&mut store, (in_ptr as i32, input_bytes.len() as i32)) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error calling _invoke: {}", e);
                return None;
            }
        };

        let output_len: usize = if output_len_i32 < 0 {
            eprintln!("Contract returned a negative length?");
            return None;
        } else {
            output_len_i32 as usize
        };

        //
        // 5) Read the output bytes back from memory. 
        //    Suppose the contract wrote them starting at some known offset (e.g. 1024). 
        //    You must match the actual logic in your contract's `_invoke`.
        //
        let out_ptr = 1024;
        let mut out_buf = vec![0u8; output_len];
        if let Err(e) = memory.read(&store, out_ptr, &mut out_buf) {
            eprintln!("Failed reading contract output from guest memory: {}", e);
            return None;
        }

        //
        // 6) Deserialize into your Output type.
        //
        let parsed_output = match serde_json::from_slice::<Output>(&out_buf) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Cannot parse contract output as JSON: {}", e);
                return None;
            }
        };

        Some(parsed_output)
    }

    pub async fn get_contracts(
        &self,
        args: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> anyhow::Result<Vec<Option<ContractRecord>>> {
        let mut client: Client = self.db_pool.get().await?;
        let txn = client.transaction().await?;

        let stmt = txn.prepare(
            "SELECT is_contract, in_contract, at_address, data, version FROM contracts WHERE in_contract = $1 AND at_address = $2"
        ).await?;

        let mut records = Vec::new();

        for (in_contract, at_address) in args {
            let record = txn.query_opt(&stmt, &[&in_contract, &at_address]).await?.map(|row| ContractRecord {
                is_contract: row.get(0),
                in_contract: row.get(1),
                at_address: row.get(2),
                data: row.get(3),
                version: row.get(4),
            });
            records.push(record);
        }

        txn.commit().await?;
        Ok(records)
    }

    pub async fn set_contracts(&self, args: Vec<ContractSetArg>) -> anyhow::Result<Vec<Option<()>>> {
        let mut client: Client = self.db_pool.get().await?;
        let txn = client.transaction().await?;

        let stmt = txn.prepare(
            "INSERT INTO contracts (is_contract, in_contract, at_address, data, version)
             VALUES ($1, $2, $3, $4, 0)
             ON CONFLICT (in_contract, at_address)
             DO UPDATE SET
               is_contract = EXCLUDED.is_contract,
               data = EXCLUDED.data,
               version = contracts.version + 1
             WHERE contracts.version = COALESCE($5, contracts.version)"
        ).await?;

        let mut results = Vec::new();

        for arg in args {
            let rows_affected = txn.execute(
                &stmt,
                &[&arg.is_contract as &(dyn ToSql + Sync), &arg.in_contract, &arg.at_address, &arg.data, &arg.version],
            ).await?;

            results.push(if rows_affected == 1 { Some(()) } else { None });
        }

        txn.commit().await?;
        Ok(results)
    }
}
