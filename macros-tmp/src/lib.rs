use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn};

#[proc_macro_attribute]
pub fn entrypoint(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Ensure the function has exactly one argument.
    if input_fn.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input_fn.sig.inputs,
            "Entrypoint function must have exactly one argument of type Input"
        )
        .to_compile_error()
        .into();
    }

    // Extract the function name.
    let fn_name = input_fn.sig.ident.clone();

    // The wrapper function will be exported as "enter_contract".
    let wrapper_name = syn::Ident::new("enter_contract", fn_name.span());
    let wrapper_name_lit = syn::LitStr::new("enter_contract", fn_name.span());

    // Get the type of the single argument (expected to be Input).
    let first_arg = input_fn.sig.inputs.first().unwrap();
    let arg_ty = match first_arg {
        FnArg::Typed(pat_type) => &*pat_type.ty,
        _ => {
            return syn::Error::new_spanned(
                first_arg,
                "Unexpected argument type"
            )
            .to_compile_error()
            .into();
        }
    };

    let expanded = quote! {
        // The original function remains unchanged.
        #input_fn

        // Generated wrapper that the wasm host will call.
        #[no_mangle]
        #[unsafe(export_name = #wrapper_name_lit)]
        pub unsafe extern "C" fn #wrapper_name(arg_ptr: u32, arg_len: u32) -> u64 {
            use serde_json;
            // Create a byte slice from the raw pointer.
            let data = std::slice::from_raw_parts(arg_ptr as *const u8, arg_len as usize);
            let result: std::result::Result<_, Box<dyn std::error::Error>> = (|| {
                // Deserialize the incoming JSON into the expected input type.
                let input: #arg_ty = serde_json::from_slice(data)?;
                // Call the original function.
                let output = #fn_name(input);
                Ok(output)
            })();
            match result {
                Ok(val) => {
                    // Serialize the output to a JSON string.
                    let json = serde_json::to_string(&val).unwrap();
                    let bytes = json.as_bytes();
                    // Use an external allocation function to allocate space for the result.
                    extern "C" {
                        fn alloc(size: u32) -> *mut u8;
                    }
                    let ptr = alloc(bytes.len() as u32);
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
                    // Pack the pointer (upper 32 bits) and length (lower 32 bits) into a u64.
                    ((ptr as u64) << 32) | (bytes.len() as u32 as u64)
                },
                Err(e) => {
                    eprintln!("Error in enter_contract: {:?}", e);
                    0
                }
            }
        }
    };

    TokenStream::from(expanded)
}
