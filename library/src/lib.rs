#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use serde::Deserialize;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::borrow::Borrow;
use std::hash::Hash;


pub type bytea = Vec<u8>;
pub type address = bytea;

/// A point is now a tuple (pointer, info) where for key buffers the second value is the total byte length,
/// and for value buffers the second value is a pointer to an array of per‐item lengths.
pub type point = (u32, u32);

unsafe extern "C" {
    #[link_name = "rand"]
    fn host_rand(ret_ptr: u32, ret_len_ptr: u32) -> i32;
}

/// Calls the host's get_rand and returns the resulting U256.
pub fn self_rand() -> bytea {
    let mut buf = [0u8; 32];
    let mut len: u32 = 0;
    unsafe {
        let status = host_rand(buf.as_mut_ptr() as u32, &mut len as *mut u32 as u32);
        if status != 0 {
            panic!("host_rand failed with status {}", status);
        }
        if len as usize != buf.len() {
            panic!("Expected 32 bytes for random U256, got {}", len);
        }
    }
    buf.to_vec()
}

/// Export an allocation function that the host can call.
/// This uses Rust's standard allocation routines which are backed by wee_alloc.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: u32) -> *mut u8 {
    let layout = std::alloc::Layout::from_size_align(size as usize, 1)
        .expect("failed to create layout");
    unsafe { std::alloc::alloc(layout) }
}

/// Export a deallocation function so the host can free memory.
#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: *mut u8, size: u32) {
    let layout = std::alloc::Layout::from_size_align(size as usize, 1)
        .expect("failed to create layout");
    unsafe { std::alloc::dealloc(ptr, layout) }
}

//
// --- Host Function Definitions ---
//

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// Batch reads data from storage.
    ///
    /// - `keys_ptr`: a contiguous buffer of keys, with the total byte length as the second tuple element.
    /// - `rets_ptr`: a tuple where the first element is a pointer to a pre-allocated data buffer that will receive
    ///    the concatenated JSON strings, and the second element is a pointer to a pre-allocated array of u32 values,
    ///    each representing the length of the corresponding returned value.
    fn gets(keys_ptr: point, rets_ptr: point) -> i32;

    /// Batch writes data to storage.
    ///
    /// - `keys_ptr`: a contiguous buffer of keys, with the total byte length.
    /// - `vals_ptr`: a tuple where the first element is a pointer to the concatenated JSON strings,
    ///    and the second element is a pointer to an array of u32 values representing the length of each JSON value.
    fn sets(keys_ptr: point, vals_ptr: point) -> i32;
}

//
// --- Error Type and Helper Functions ---
//

#[derive(Debug)]
pub enum HostLibError {
    HostGetError(i32),
    HostSetError(i32),
    SerializeError(String),
    DeserializeError(String),
    BufferTooSmall,
}

pub type Result<T> = std::result::Result<T, HostLibError>;

/// Batch reads and deserializes values of type `T` from storage (stored as JSON).
/// Assumes all keys are of fixed length.
/// The host returns a concatenated data buffer and an array of lengths (one per key).
pub fn reads<T: DeserializeOwned>(keys: Vec<bytea>) -> Vec<Option<T>> {
    let num_keys = keys.len();
    if num_keys == 0 {
        return Vec::new();
    }
    // Assume all keys are the same length.
    let key_size = keys[0].len();
    for key in &keys {
        if key.len() != key_size {
            panic!("All keys must have the same length for batch operations");
        }
    }
    // Pack keys into a contiguous buffer.
    let mut keys_buf = Vec::with_capacity(num_keys * key_size);
    for key in &keys {
        keys_buf.extend_from_slice(key);
    }
    // Allocate an output buffer for concatenated JSON values.
    // Adjust the size as needed – here we assume each value is at most 1024 bytes.
    let data_buf_size = 1024 * num_keys;
    let mut data_buf = vec![0u8; data_buf_size];
    // Allocate a vector to hold the length of each returned JSON string.
    // We need a buffer of u32 values.
    let mut lengths: Vec<u32> = vec![0; num_keys];

    // Build our two points.
    let keys_point: point = (keys_buf.as_ptr() as u32, keys_buf.len() as u32);
    // For the return, we pass the pointer to our data buffer and the pointer to our lengths array.
    let rets_point: point = (data_buf.as_mut_ptr() as u32, lengths.as_mut_ptr() as u32);

    let ret = unsafe { gets(keys_point, rets_point) };
    if ret != 0 {
        panic!("Batch gets failed with status {}", ret);
    }

    // Now, use the lengths array to extract each value.
    let mut results = Vec::with_capacity(num_keys);
    let mut offset = 0usize;
    for &len in &lengths {
        if len == 0 {
            results.push(None);
        } else {
            if offset + (len as usize) > data_buf.len() {
                panic!(
                    "Data buffer overrun: offset {} + length {} exceeds buffer size {}",
                    offset,
                    len,
                    data_buf.len()
                );
            }
            let slice = &data_buf[offset..offset + (len as usize)];
            let json_str = std::str::from_utf8(slice)
                .unwrap_or_else(|e| panic!("Invalid UTF-8 in returned data: {}", e));
            let value: T = serde_json::from_str(json_str)
                .unwrap_or_else(|e| panic!("Deserialization failed: {}", e));
            results.push(Some(value));
            offset += len as usize;
        }
    }
    results
}

/// Batch serializes values of type `T` to JSON and writes them to storage.
/// This function packs the keys and the corresponding JSON values in one call.
/// It expects a pair of vectors: one for keys and one for values.
pub fn writes<T: Serialize>(data: (Vec<bytea>, Vec<T>)) {
    let (keys, values) = data;
    let num_keys = keys.len();
    if num_keys == 0 {
        return;
    }
    if values.len() != num_keys {
        panic!("Number of keys and values must be equal");
    }
    // Assume keys are fixed length.
    let key_size = keys[0].len();
    for key in &keys {
        if key.len() != key_size {
            panic!("All keys must have the same length for batch operations");
        }
    }
    // Pack keys into a contiguous buffer.
    let mut keys_buf = Vec::with_capacity(num_keys * key_size);
    for key in &keys {
        keys_buf.extend_from_slice(key);
    }
    // For each value, serialize to JSON and pack the results into a single buffer while
    // collecting each JSON's length.
    let mut vals_buf = Vec::new();
    let mut lengths: Vec<u32> = Vec::with_capacity(num_keys);
    for value in &values {
        let json_str = serde_json::to_string(value)
            .unwrap_or_else(|e| panic!("Serialization failed: {}", e));
        let bytes = json_str.as_bytes();
        lengths.push(bytes.len() as u32);
        vals_buf.extend_from_slice(bytes);
    }

    let keys_point: point = (keys_buf.as_ptr() as u32, keys_buf.len() as u32);
    // For the values, we send the pointer to the concatenated JSON bytes and a pointer
    // to the lengths array.
    let vals_point: point = (vals_buf.as_ptr() as u32, lengths.as_mut_ptr() as u32);

    let ret = unsafe { sets(keys_point, vals_point) };
    if ret != 0 {
        panic!("Batch sets failed with status {}", ret);
    }
}

//
// --- Single-Key Helpers and Pointer/Table Definitions ---
//

// The single-key versions remain largely unchanged – they wrap the batch calls for one item.
pub fn read_storage_json<T: DeserializeOwned>(key: &[u8]) -> Option<T> {
    // We simply call the batch version with a single key.
    let results = reads::<T>(vec![key.to_vec()]);
    results.into_iter().next().unwrap_or(None)
}

pub fn write_storage_json<T: Serialize>(key: &[u8], value: T) {
    writes::<T>((vec![key.to_vec()], vec![value]))
}

/// Pointer abstraction for (de)serializable values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pointer<T> {
    key: address,
    len: u32,
    _marker: PhantomData<T>,
}

impl<T> Pointer<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Creates a new Pointer.
    /// If a key is provided, that key is used; otherwise a random U256 is generated.
    pub fn new(key: Option<bytea>) -> Self {
        let key = key.unwrap_or_else(|| self_rand());
        Self {
            key,
            len: 32, // assuming a 32-byte key; adjust if needed
            _marker: PhantomData,
        }
    }

    /// Reads the value stored at this pointer.
    pub fn get(&self) -> Option<T> {
        read_storage_json::<T>(&self.key)
    }

    /// Writes the given value to storage.
    pub fn set(&self, value: T) -> () {
        write_storage_json::<T>(&self.key, value)
    }

    pub fn ptr(&self) -> bytea {
        self.key.clone()
    }
}

impl<T> AsRef<bytea> for Pointer<T> {
    fn as_ref(&self) -> &bytea {
        &self.key
    }
}

impl<T> Borrow<bytea> for Pointer<T> {
    fn borrow(&self) -> &bytea {
        &self.key
    }
}

/// Reads the root value from storage at slot 0.
pub fn get_root<T: DeserializeOwned>() -> Option<T> {
    read_storage_json::<T>(&[0; 32])
}

/// Writes the given root value to storage at slot 0.
pub fn set_root<T: Serialize>(root: T) -> () {
    write_storage_json::<T>(&[0; 32], root)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Table<K, V> {
    table_name: String,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> Table<K, V>
where
    K: Eq + Hash + Serialize,
    V: Serialize + DeserializeOwned,
{
    /// Creates a new Table instance with a specified table name.
    pub fn new(table_name: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            _marker: PhantomData,
        }
    }

    /// Computes the BLAKE3 hash of the input bytes and returns it as a Vec<u8>.
    fn blake3_hash(input: &[u8]) -> Vec<u8> {
        blake3::hash(input).as_bytes().to_vec()
    }

    /// Generates a hashed storage key for the given logical key.
    fn storage_key(&self, key: &K) -> Vec<u8> {
        let compound_key = serde_json::to_string(&(self.table_name.as_str(), key))
            .expect("Failed to serialize key");
        Self::blake3_hash(&compound_key.as_bytes())
    }

    /// Retrieves a Pointer<V> for the provided key.
    pub fn get(&self, key: &K) -> Pointer<V> {
        Pointer::new(Some(self.storage_key(key)))
    }

    /// Stores the value associated with the provided key.
    pub fn set(&self, key: &K, value: V) {
        let pointer = Pointer::new(Some(self.storage_key(key)));
        pointer.set(value);
    }
}

pub fn verify_commit_reveal<T: Serialize + Eq>(commit: Vec<u8>, reveal: T) -> bool {
    // Serialize the revealed value into JSON bytes
    let serialized_reveal = serde_json::to_vec(&reveal);

    // Return false if serialization fails
    let serialized_reveal = match serialized_reveal {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };

    // Compute the blake3 hash of the serialized reveal
    let hash = blake3::hash(&serialized_reveal);

    // Compare the computed hash with the provided commit
    commit == hash.as_bytes()
}


/* EXECUTION */
#[derive(Serialize, Deserialize, Clone)]
pub struct Input {
    pub contract: Vec<u8>,
    pub functions: HashMap<String, Vec<Value>>
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Output {
    pub functions: HashMap<String, Vec<Option<Value>>>
}

#[derive(Serialize, Deserialize)]
pub struct Invoke {
    identity: Vec<u8>,
    pub payload: InvokePayload
}

#[derive(Serialize, Deserialize)]
pub struct InvokePayload {
    contract: Vec<u8>,
    function: String,
    pub argument: Value
}

// Define an extension trait to add the `contract` method.
pub trait InputExt {
    fn contract(&self) -> Vec<Input>;
}

impl InputExt for Vec<Invoke> {
    fn contract(&self) -> Vec<Input> {
        // This HashMap groups invokes by contract.
        let mut grouped: HashMap<Vec<u8>, HashMap<String, Vec<Value>>> = HashMap::new();
        // This vector records the order in which each unique contract first appears.
        let mut order: Vec<Vec<u8>> = Vec::new();

        for invoke in self.iter() {
            // If this contract hasn't been seen before, record its order.
            if !grouped.contains_key(&invoke.payload.contract) {
                order.push(invoke.payload.contract.clone());
                grouped.insert(invoke.payload.contract.clone(), HashMap::new());
            }
            // Insert the function and argument into the grouped map.
            grouped
                .get_mut(&invoke.payload.contract)
                .unwrap()
                .entry(invoke.payload.function.clone())
                .or_insert_with(Vec::new)
                .push(invoke.payload.argument.clone());
        }

        // Build the resulting vector of Contractions in the order the contracts first appeared.
        let mut result = Vec::new();
        for contract in order {
            if let Some(functions) = grouped.remove(&contract) {
                result.push(Input { contract, functions });
            }
        }
        result
    }
}