pub fn vec_prefix_to_u32(v: &Vec<u8>) -> Option<u32> {
    if v.len() < 8 || v.len() - 8 > 4 {
        // Prefix length is invalid (must be <= 4 bytes for u32)
        return None;
    }
    let prefix = &v[..v.len() - 8];

    // Convert prefix bytes into u32, assuming Big Endian
    let mut bytes = [0u8; 4];
    bytes[4 - prefix.len()..].copy_from_slice(prefix);
    Some(u32::from_be_bytes(bytes))
}

pub fn u32_to_qinfinity64(n: u32) -> Vec<u8> {
    const FRACTIONAL_BITS: usize = 64;
    // If n is zero, return [0] as the minimal representation.
    if n == 0 {
        return vec![0];
    }
    // Multiply by 2^64 by shifting left.
    let value: u128 = (n as u128) << FRACTIONAL_BITS;
    // Convert the value to a little-endian byte array.
    let mut result = Vec::new();
    let mut temp = value;
    while temp > 0 {
        result.push((temp & 0xff) as u8);
        temp >>= 8;
    }
    result
}

/// Computes the multiplicative inverse of a Qinfinity.64 fixed‑point number
/// (represented as a little‑endian byte array) using Newton–Raphson iteration.
/// 
/// # Panics
/// 
/// Panics if `input` represents zero.
/// 
/// # Example
/// 
/// ```
/// // For example, the inverse of 2 (in fixed‑point Qinfinity.64) should be approximately 0.5.
/// let two_fp = fixed_point_two(); // 2 in fixed‑point (2 << 64)
/// let inv = bytearray_inverse(&two_fp);
/// // The fixed‑point representation of 1 is 1 << 64, so 0.5 should be roughly (1 << 63).
/// ```
pub fn bytearray_inverse(input: &Vec<u8>) -> Vec<u8> {
    // It is the caller’s responsibility not to call this with a zero input.
    // We use Newton–Raphson to solve for y in: multiply_byte_arrays(input, y) ≈ one,
    // where one (in fixed‑point) is 1 << FRACTIONAL_BITS.
    //
    // The iteration is:
    //     yₙ₊₁ = multiply_byte_arrays(yₙ, subtract_bytearrays(two, multiply_byte_arrays(input, yₙ)))
    //
    // Here, "two" is the fixed‑point representation of 2 (i.e. 2 << FRACTIONAL_BITS).
    //
    // First, choose an initial guess. One strategy is to use a power‑of‑two whose exponent
    // is (128 – bit_length(input)). (Because if the true reciprocal is 2^128/input, then a power‐of‐two is a rough start.)
    let bits = bit_length(input);
    let init_exp = if 128 > bits { 128 - bits } else { 0 };
    let mut y = power_of_two(init_exp);
    let two = fixed_point_two(); // represents 2 in Qinfinity.64 (i.e. 2 << 64)

    // Perform several iterations; Newton–Raphson converges quadratically.
    for _ in 0..10 {
        let prod = multiply_byte_arrays(input, &y); // x * y
        let diff = subtract_bytearrays(&two, &prod);   // (2 - x * y)
        y = multiply_byte_arrays(&y, &diff);           // y * (2 - x * y)
    }
    y
}

/// Subtracts two little‑endian Qinfinity.64 numbers: returns a – b.
/// Assumes a ≥ b.
fn subtract_bytearrays(a: &[u8], b: &[u8]) -> Vec<u8> {
    let max_len = a.len().max(b.len());
    let mut result = vec![0u8; max_len];
    let mut borrow = 0i16;
    for i in 0..max_len {
        let ai = *a.get(i).unwrap_or(&0) as i16;
        let bi = *b.get(i).unwrap_or(&0) as i16;
        let mut diff = ai - bi - borrow;
        if diff < 0 {
            diff += 256;
            borrow = 1;
        } else {
            borrow = 0;
        }
        result[i] = diff as u8;
    }
    // Remove any unnecessary leading zero bytes (keeping at least one byte).
    while result.len() > 1 && *result.last().unwrap() == 0 {
        result.pop();
    }
    result
}

/// Returns the number of bits required to represent the number in `a`
/// (where `a` is interpreted as a little‑endian unsigned integer).
fn bit_length(a: &[u8]) -> usize {
    if let Some(&msb) = a.last() {
        let bits_in_msb = 8 - msb.leading_zeros() as usize;
        (a.len() - 1) * 8 + bits_in_msb
    } else {
        0
    }
}

/// Returns a little‑endian representation of 2^exp.
fn power_of_two(exp: usize) -> Vec<u8> {
    let num_bytes = exp / 8 + 1;
    let mut result = vec![0u8; num_bytes];
    let byte_index = exp / 8;
    let bit_index = exp % 8;
    result[byte_index] = 1u8 << bit_index;
    result
}

/// Returns the fixed‑point representation of 2 (i.e. 2 << FRACTIONAL_BITS).
/// Since FRACTIONAL_BITS is 64, 2 is 2^65, which requires 9 bytes in little‑endian.
fn fixed_point_two() -> Vec<u8> {
    let mut result = vec![0u8; 9];
    result[8] = 2;
    result
}

pub fn multiply_byte_arrays(a: &Vec<u8>, b: &Vec<u8>) -> Vec<u8> {
    const FRACTIONAL_BITS: usize = 64;
    let mut result = vec![0u8; a.len() + b.len()];

    // Perform multiplication byte by byte.
    for (i, &byte_a) in a.iter().enumerate() {
        let mut carry = 0u16;
        for (j, &byte_b) in b.iter().enumerate() {
            let index = i + j;
            let existing = result[index] as u16;
            let product = byte_a as u16 * byte_b as u16 + existing + carry;
            result[index] = product as u8;
            carry = product >> 8;
        }
        let mut index = i + b.len();
        while carry > 0 {
            let total = result[index] as u16 + carry;
            result[index] = total as u8;
            carry = total >> 8;
            index += 1;
        }
    }

    // Adjust result for fixed-point by shifting right FRACTIONAL_BITS bits
    for _ in 0..FRACTIONAL_BITS / 8 {
        result.remove(0);
    }

    while result.len() > 1 && result.last() == Some(&0) {
        result.pop();
    }

    result
}

// Calculates the N-th root (1/N power) of a Qinfinity.64 fixed-point byte array input using binary search.
pub fn bytearray_root(input: &Vec<u8>, n: u32) -> Vec<u8> {
    const FRACTIONAL_BITS: usize = 64;
    let mut low = vec![0u8];
    let mut high = input.to_vec();

    for _ in 0..FRACTIONAL_BITS * 2 {
        let mid = divide_by_two(&add_bytearrays(&low, &high));
        let mid_pow_n = bytearray_pow(&mid, n);

        if mid_pow_n <= input.to_vec() {
            low = mid;
        } else {
            high = mid;
        }
    }

    low
}

// Adds two Qinfinity.64 fixed-point byte arrays representing little-endian numbers.
pub fn add_bytearrays(a: &Vec<u8>, b: &Vec<u8>) -> Vec<u8> {
    let max_len = a.len().max(b.len());
    let mut result = vec![0u8; max_len + 1];
    let mut carry = 0u16;

    for i in 0..max_len {
        let byte_a = *a.get(i).unwrap_or(&0) as u16;
        let byte_b = *b.get(i).unwrap_or(&0) as u16;
        let sum = byte_a + byte_b + carry;
        result[i] = sum as u8;
        carry = sum >> 8;
    }

    if carry > 0 {
        result[max_len] = carry as u8;
    } else {
        result.pop();
    }

    result
}

// Divides a Qinfinity.64 fixed-point byte array by two.
fn divide_by_two(a: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; a.len()];
    let mut carry = 0;
    for (i, &byte) in a.iter().enumerate().rev() {
        result[i] = (byte >> 1) | carry;
        carry = (byte & 1) << 7;
    }
    while result.len() > 1 && result.last() == Some(&0) {
        result.pop();
    }
    result
}

// Raises a Qinfinity.64 fixed-point byte array to a power.
fn bytearray_pow(base: &Vec<u8>, exponent: u32) -> Vec<u8> {
    let mut result = vec![1u8];
    let mut base_pow = base.to_vec();
    let mut exp = exponent;

    while exp > 0 {
        if exp % 2 == 1 {
            result = multiply_byte_arrays(&result, &base_pow);
        }
        base_pow = multiply_byte_arrays(&base_pow, &base_pow);
        exp /= 2;
    }

    result
}
