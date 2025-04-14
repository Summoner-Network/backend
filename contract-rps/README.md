To build the RPS contract (located in the `contract-rps` folder) as a WebAssembly module using the `wasm32-unknown-unknown` target, you can follow these steps:

---

## 1. Install the WASM Target

Before you compile your contract, make sure that the WebAssembly target is installed in your Rust toolchain. You can install it with:

```bash
rustup target add wasm32-unknown-unknown
```

---

## 2. Build the Contract

Once the target is installed, navigate to your workspace root (or directly to the `contract-rps` directory if you prefer) and run the following cargo build command. This command tells Cargo to compile the `contract-rps` crate for the `wasm32-unknown-unknown` target in release mode:

```bash
cargo build --target wasm32-unknown-unknown --release -p contract-rps
```

- **`--target wasm32-unknown-unknown`**: Specifies the WASM compilation target.
- **`--release`**: Builds the contract in release mode (optimizations enabled).
- **`-p contract-rps`**: Limits the build to the `contract-rps` crate within the workspace.

---

## 3. Locate the Output WASM File

After the build completes successfully, the generated WebAssembly file will be found in the target directory. The typical path is:

```
target/wasm32-unknown-unknown/release/contract_rps.wasm
```

(Notice that Cargo converts dashes to underscores in the output file name.)

---

## 4. (Optional) Optimize Your WASM Binary

WASM binaries generated with Rust can sometimes be a bit large. You might consider using tools like `wasm-opt` (from [Binaryen](https://github.com/WebAssembly/binaryen)) to optimize your WASM file further:

```bash
wasm-opt -O3 target/wasm32-unknown-unknown/release/contract_rps.wasm -o contract_rps_optimized.wasm
```

This step is optional and depends on your performance and size requirements.

---

## Summary

1. **Add the WASM target:**
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. **Build the contract:**
   ```bash
   cargo build --target wasm32-unknown-unknown --release -p contract-rps
   ```

3. **Find your WASM binary in:**
   ```
   target/wasm32-unknown-unknown/release/contract_rps.wasm
   ```

4. **(Optionally) Optimize using `wasm-opt`.**

By following these steps, you will have built your RPS contract as a WASM module using the `wasm32-unknown-unknown` target.