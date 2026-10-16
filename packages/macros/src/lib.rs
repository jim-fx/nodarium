extern crate proc_macro;
use nodarium_types::NodeDefinition;
use proc_macro::TokenStream;
use quote::quote;
use std::env;
use std::fs;
use std::path::Path;
use syn::parse_macro_input;

fn add_line_numbers(input: String) -> String {
    return input
        .split('\n')
        .enumerate()
        .map(|(i, line)| format!("{:2}: {}", i + 1, line))
        .collect::<Vec<String>>()
        .join("\n");
}

#[proc_macro_attribute]
pub fn nodarium_execute(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut inner_fn = parse_macro_input!(item as syn::ItemFn);
    let inner_ident = syn::Ident::new("__nodarium_execute_inner", inner_fn.sig.ident.span());
    inner_fn.sig.ident = inner_ident.clone();

    // Exports the Nodarium ABI v1 (see docs/ABI.md) around the user function,
    // which receives one slice per input: fn execute(args: &[&[i32]]) -> Vec<i32>
    let expanded = quote! {
        #inner_fn

        #[no_mangle]
        pub extern "C" fn nodarium_alloc(bytes: usize) -> *mut i32 {
            nodarium_utils::abi::alloc(bytes)
        }

        #[no_mangle]
        pub extern "C" fn nodarium_reset() {
            nodarium_utils::abi::reset()
        }

        #[no_mangle]
        pub extern "C" fn nodarium_execute(args_ptr: *const u32, argc: usize) -> *const u32 {
            nodarium_utils::abi::setup_panic_hook();
            let args = unsafe { nodarium_utils::abi::read_args(args_ptr, argc) };
            nodarium_utils::abi::store_result(#inner_ident(&args))
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro]
pub fn nodarium_definition_file(input: TokenStream) -> TokenStream {
    let path_lit = syn::parse_macro_input!(input as syn::LitStr);
    let file_path = path_lit.value();

    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let full_path = Path::new(&project_dir).join(&file_path);

    let json_content = fs::read_to_string(&full_path).unwrap_or_else(|err| {
        panic!("Failed to read JSON file at '{}/{}': {}", project_dir, file_path, err)
    });

    let _: NodeDefinition = serde_json::from_str(&json_content).unwrap_or_else(|err| {
        panic!("JSON file contains invalid JSON: \n{} \n{}", err, add_line_numbers(json_content.clone()))
    });

    // We use the span from the input path literal
    let bytes = syn::LitByteStr::new(json_content.as_bytes(), path_lit.span());
    let len = json_content.len();

    let expanded = quote! {
        #[link_section = "nodarium_definition"]
        static DEFINITION_DATA: [u8; #len] = *#bytes;
        
        #[no_mangle]
        pub extern "C" fn get_definition_ptr() -> *const u8 {
            DEFINITION_DATA.as_ptr()
        }

        #[no_mangle]
        pub extern "C" fn get_definition_len() -> usize {
            DEFINITION_DATA.len()
        }
    };

    TokenStream::from(expanded)
}
