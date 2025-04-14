// server/src/globals.rs

use std::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::Lazy;

/// Thread-safe global program counter, starting at 0.
static PROGRAM_COUNTER: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));

/// Returns the current value of the program counter and increments it atomically.
///
/// # Returns
/// The value of the program counter **before** incrementing.
pub fn get_pc() -> u64 {
    PROGRAM_COUNTER.fetch_add(1, Ordering::SeqCst)
}