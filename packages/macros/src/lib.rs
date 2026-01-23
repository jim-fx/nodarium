extern crate proc_macro;
use nodarium_types::NodeDefinition;
use proc_macro::TokenStream;
use quote::quote;
use std::env;
use std::fs;
use std::path::Path;
use syn::parse_macro_input;
use syn::spanned::Spanned;

fn add_line_numbers(input: String) -> String {
    return input
        .split('\n')
        .enumerate()
        .map(|(i, line)| format!("{:2}: {}", i + 1, line))
        .collect::<Vec<String>>()
        .join("\n");
}

fn read_node_definition(file_path: &Path) -> NodeDefinition {
    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let full_path = Path::new(&project_dir).join(file_path);
    let json_content = fs::read_to_string(&full_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read JSON file at '{}/{}': {}",
            project_dir,
            file_path.to_string_lossy(),
            err
        )
    });
    serde_json::from_str(&json_content).unwrap_or_else(|err| {
        panic!(
            "JSON file contains invalid JSON: \n{} \n{}",
            err,
            add_line_numbers(json_content.clone())
        )
    })
}

#[proc_macro_attribute]
pub fn nodarium_execute(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_body = &input_fn.block;
    let inner_fn_name = syn::Ident::new(&format!("__nodarium_inner_{}", fn_name), fn_name.span());

    let def: NodeDefinition = read_node_definition(Path::new("src/input.json"));

    let input_count = def.inputs.as_ref().map(|i| i.len()).unwrap_or(0);

    validate_signature(&input_fn.sig, input_count, &def);

    let input_param_names: Vec<_> = input_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    Some(pat_ident.ident.clone())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    let param_count = input_fn.sig.inputs.len();
    let total_c_params = param_count * 2;

    let arg_names: Vec<_> = (0..total_c_params)
        .map(|i| syn::Ident::new(&format!("arg{}", i), input_fn.sig.span()))
        .collect();

    let mut tuple_args = Vec::new();
    for i in 0..param_count {
        let start_name = &arg_names[i * 2];
        let end_name = &arg_names[i * 2 + 1];
        let tuple_arg = quote! {
            (#start_name, #end_name)
        };
        tuple_args.push(tuple_arg);
    }

    let expanded = quote! {

        extern "C" {
            fn __nodarium_log(ptr: *const u8, len: usize);
            fn __nodarium_log_panic(ptr: *const u8, len: usize);
        }

        #fn_vis fn #inner_fn_name(#( #input_param_names: (i32, i32) ),*) -> Vec<i32> {
            #fn_body
        }

        #[no_mangle]
        #fn_vis extern "C" fn execute(output_pos: i32, #( #arg_names: i32 ),*) -> i32 {

            nodarium_utils::log!("before_fn");
            let result = #inner_fn_name(
                #( #tuple_args ),*
            );
            nodarium_utils::log!("after_fn");

            let len_bytes = result.len() * 4;
            unsafe {
                let src = result.as_ptr() as *const u8;
                let dst = output_pos as *mut u8;
                // nodarium_utils::log!("writing output_pos={:?} src={:?} len_bytes={:?}", output_pos, src, len_bytes);
                dst.copy_from_nonoverlapping(src, len_bytes);
            }

            len_bytes as i32
        }
    };

    TokenStream::from(expanded)
}

fn validate_signature(fn_sig: &syn::Signature, expected_inputs: usize, def: &NodeDefinition) {
    let param_count = fn_sig.inputs.len();
    let expected_params = expected_inputs;

    if param_count != expected_params {
        panic!(
            "Execute function has {} parameters but definition has {} inputs\n\
             Definition inputs: {:?}\n\
             Expected signature:\n\
             pub fn execute({}) -> Vec<i32>",
            param_count,
            expected_inputs,
            def.inputs
                .as_ref()
                .map(|i| i.keys().collect::<Vec<_>>())
                .unwrap_or_default(),
            (0..expected_inputs)
                .map(|i| format!("arg{}: (i32, i32)", i))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    for (i, arg) in fn_sig.inputs.iter().enumerate() {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let type_str = quote! { #pat_type.ty }.to_string();
                let clean_type = type_str
                    .trim()
                    .trim_start_matches("_")
                    .trim_end_matches(".ty")
                    .trim()
                    .to_string();
                if !clean_type.contains("(") && !clean_type.contains(",") {
                    panic!(
                        "Parameter {} has type '{}' but should be a tuple (i32, i32) representing (start, end) positions in memory",
                        i,
                        clean_type
                    );
                }
            }
            syn::FnArg::Receiver(_) => {
                panic!("Execute function cannot have 'self' parameter");
            }
        }
    }

    match &fn_sig.output {
        syn::ReturnType::Type(_, ty) => {
            let is_vec = match &**ty {
                syn::Type::Path(tp) => tp
                    .path
                    .segments
                    .first()
                    .map(|seg| seg.ident == "Vec")
                    .unwrap_or(false),
                _ => false,
            };
            if !is_vec {
                panic!("Execute function must return Vec<i32>");
            }
        }
        syn::ReturnType::Default => {
            panic!("Execute function must return Vec<i32>");
        }
    }
}

#[proc_macro]
pub fn nodarium_definition_file(input: TokenStream) -> TokenStream {
    let path_lit = syn::parse_macro_input!(input as syn::LitStr);
    let file_path = path_lit.value();

    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let full_path = Path::new(&project_dir).join(&file_path);

    let json_content = fs::read_to_string(&full_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read JSON file at '{}/{}': {}",
            project_dir, file_path, err
        )
    });

    let _: NodeDefinition = serde_json::from_str(&json_content).unwrap_or_else(|err| {
        panic!(
            "JSON file contains invalid JSON: \n{} \n{}",
            err,
            add_line_numbers(json_content.clone())
        )
    });

    let bytes = syn::LitByteStr::new(json_content.as_bytes(), path_lit.span());
    let len = json_content.len();

    let expanded = quote! {
        #[link_section = "nodarium_definition"]
        static DEFINITION_DATA: [u8; #len] = *#bytes;
    };

    TokenStream::from(expanded)
}
