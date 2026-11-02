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

fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn names_not_in_snake_case(definition: &NodeDefinition) -> Vec<String> {
    let segments = definition.id.split('/').map(str::to_string);
    let inputs = definition.inputs.iter().flat_map(|inputs| inputs.keys().cloned());
    let mut names: Vec<String> = segments.chain(inputs).filter(|name| !is_snake_case(name)).collect();
    names.sort();
    names
}

#[proc_macro_attribute]
pub fn nodarium_execute(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut inner_fn = parse_macro_input!(item as syn::ItemFn);
    let inner_ident = syn::Ident::new("__nodarium_execute_inner", inner_fn.sig.ident.span());
    inner_fn.sig.ident = inner_ident.clone();

    // Exports the Nodarium ABI v1 (see docs/ABI.md) around the user function,
    // which receives one slice per input: fn execute(args: &[&[i32]]) -> impl NodeOutputs
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

    let definition: NodeDefinition = serde_json::from_str(&json_content).unwrap_or_else(|err| {
        panic!("JSON file contains invalid JSON: \n{} \n{}", err, add_line_numbers(json_content.clone()))
    });

    let invalid = names_not_in_snake_case(&definition);
    if !invalid.is_empty() {
        panic!(
            "Node ids and input names in '{}' must be snake_case (lowercase letters, digits and _, starting with a letter): {}",
            file_path,
            invalid.join(", ")
        );
    }

    // We use the span from the input path literal
    let bytes = syn::LitByteStr::new(json_content.as_bytes(), path_lit.span());
    let len = json_content.len();
    let full_path_str = full_path.to_string_lossy().to_string();

    let expanded = quote! {
        // the file is read above, include it too so cargo rebuilds the node when it changes
        const _: &[u8] = include_bytes!(#full_path_str);

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

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(json: &str) -> NodeDefinition {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn accepts_snake_case() {
        let def = definition(r#"{"id":"max/plantarium/trunk_kink","inputs":{"lowest_kink":{"type":"float"},"x2":{"type":"float"}}}"#);
        assert!(names_not_in_snake_case(&def).is_empty());
    }

    #[test]
    fn rejects_other_names() {
        let def = definition(
            r#"{"id":"max/plantarium/trunkKink","inputs":{"lowestKink":{"type":"float"},"0":{"type":"float"},"_a":{"type":"float"},"ok":{"type":"float"}}}"#,
        );
        assert_eq!(names_not_in_snake_case(&def), ["0", "_a", "lowestKink", "trunkKink"]);
    }
}
