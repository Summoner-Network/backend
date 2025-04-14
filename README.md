# InvokeOS

![InvokeOS Logo](summoner-logo.png)

InvokeOS is a next-generation smart contracting platform built on the idea of **Convolutional Smart Contracts**. By processing all transactions for a given contract as a single unit within a block instead of one-by-one, InvokeOS is able to:
- **Support Higher Throughput:** Significantly increase the number of transactions processed concurrently.
- **Improve Execution Quality:** Enable more efficient and effective execution of smart contracts.
- **Reduce Resource Usage:** Minimize the resource consumption by netting and grouping transactions.
- **Introduce New Execution Paradigms:** For example, advanced netting techniques for transaction processing.

---

## Overview

The project is organized as a Cargo workspace with several interrelated crates:

- **contract-rps:** Contains the core logic and library for contract processing, including smart contract primitives.
- **library:** Provides supporting functionality such as input/output processing, signature verification, and contract invocation helpers.
- **macros:** Contains helper macros used across the project.
- **server:** The server application that exposes HTTP endpoints for deploying and invoking contracts. It also includes:
  - **Batched Processing:** A custom asynchronous batching system (`AsyncBatchWorker`) is used in `invoke_batcher.rs` to optimize contract invocation.
  - **Database Interaction:** Uses PostgreSQL via `deadpool-postgres` to store contract data.
  - **Wasm Integration:** Uses Wasmtime to compile and execute Wasm modules of smart contracts.
  - **Global Utilities:** Includes modules for low-level byte math, a global program counter, and additional utilities.

---

## Key Features

- **Convolutional Batch Processing:**  
  All incoming invoke requests are collected into batches, flattened into a single batch of `Invoke` items, and processed concurrently. The results are then deconvoluted back to their original submissions.

- **Asynchronous Processing:**  
  Uses Tokio and futures to process batched transactions concurrently for increased scalability.

- **Smart Contract Deployment:**  
  Deploy new contracts by computing their cryptographic hash and upserting them into a PostgreSQL database.

- **Web Assembly (Wasm) Execution:**  
  Supports smart contract execution as Wasm modules using Wasmtime.

- **Advanced ByteMath Functions:**  
  Provides utilities for fixed-point arithmetic needed for contract computations.

---

## Getting Started

### Prerequisites

- **Rust Toolchain (Nightly recommended for some components):**  
  Install via [rustup](https://rustup.rs/).

- **PostgreSQL:**  
  Ensure you have PostgreSQL installed; you can also use Docker Compose (see below).

- **Docker (optional):**  
  For containerized deployment.

### Building the Project

#### Using Cargo

At the root of the workspace, run:

```bash
cargo build --release
```

This will build all the crates in the workspace.

#### Using Docker Compose

The project includes a `docker-compose.yml` file that spins up:
- A PostgreSQL service.
- The InvokeOS server.

Run the following command in the root directory:

```bash
docker-compose up --build
```

This command builds the server image using the `server/Dockerfile`, starts the PostgreSQL container, and runs the server. The server will be available on port 8080.

---

## Running the Server

Once built, the server provides HTTP endpoints for deploying and invoking smart contracts.

- **Deploy Contract:**  
  `POST /deploy`  
  Upload the contract bytes. The server computes the contract hash and upserts the contract code in the database.

- **Invoke Contract:**  
  `POST /invoke`  
  Submit a JSON payload containing a vector of `Invoke` items. The server batches the request, processes the smart contract via Wasmtime, and returns the results as a JSON response.

Example payload for `/invoke`:

```json
{
    "call": [
        { /* Invoke data... */ },
        { /* Another invoke... */ }
    ]
}
```

The response will be a vector of responses like:

```json
[
  {
    "emit": [ /* Resulting JSON values... */ ]
  }
]
```

---

## Project Structure

```plaintext
├── contract-rps
│   ├── src
│   │   └── lib.rs
│   ├── Cargo.toml
│   └── README.md
├── library
│   ├── src
│   │   └── lib.rs
│   └── Cargo.toml
├── macros
│   ├── src
│   │   └── lib.rs
│   └── Cargo.toml
├── server
│   ├── sql
│   │   └── genesis.sql
│   ├── src
│   │   ├── __deploy__.rs
│   │   ├── __invoke__.rs
│   │   ├── batches.rs
│   │   ├── bytemath.rs
│   │   ├── globals.rs
│   │   ├── invoke_batcher.rs
│   │   ├── main.rs
│   │   ├── structs.rs
│   │   └── traits.rs
│   ├── Cargo.lock
│   ├── Cargo.toml
│   └── Dockerfile
├── Cargo.lock
├── Cargo.toml
├── docker-compose.yml
├── README.md
└── summoner-logo.png
```

---

## Contributing

Contributions are welcome! Please open an issue or submit a pull request with your suggestions or bug fixes.

---

## License

[MIT License](LICENSE)

---

This README provides an overview of the project, instructions to get started, and details about its architecture and key features. Adjust or expand the documentation as necessary to suit your development and deployment workflow.