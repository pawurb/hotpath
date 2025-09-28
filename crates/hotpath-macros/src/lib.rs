use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{parse_macro_input, ItemFn, LitInt, LitStr};

#[derive(Clone, Copy)]
enum Format {
    Table,
    Json,
    JsonPretty,
}

impl Format {
    fn to_tokens(self) -> proc_macro2::TokenStream {
        match self {
            Format::Table => quote!(hotpath::Format::Table),
            Format::Json => quote!(hotpath::Format::Json),
            Format::JsonPretty => quote!(hotpath::Format::JsonPretty),
        }
    }
}

#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    // Defaults
    let mut percentiles: Vec<u8> = vec![95];
    let mut format = Format::Table;

    // Parse named args like: percentiles=[..], format=".."
    if !attr.is_empty() {
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("percentiles") {
                meta.input.parse::<syn::Token![=]>()?;
                let content;
                syn::bracketed!(content in meta.input);
                let mut vals = Vec::new();
                while !content.is_empty() {
                    let li: LitInt = content.parse()?;
                    let v: u8 = li.base10_parse()?;
                    if !(0..=100).contains(&v) {
                        return Err(
                            meta.error(format!("Invalid percentile {} (must be 0..=100)", v))
                        );
                    }
                    vals.push(v);
                    if !content.is_empty() {
                        content.parse::<syn::Token![,]>()?;
                    }
                }
                if vals.is_empty() {
                    return Err(meta.error("At least one percentile must be specified"));
                }
                percentiles = vals;
                return Ok(());
            }

            if meta.path.is_ident("format") {
                meta.input.parse::<syn::Token![=]>()?;
                let lit: LitStr = meta.input.parse()?;
                format =
                    match lit.value().as_str() {
                        "table" => Format::Table,
                        "json" => Format::Json,
                        "json-pretty" => Format::JsonPretty,
                        other => return Err(meta.error(format!(
                            "Unknown format {:?}. Expected one of: \"table\", \"json\", \"json-pretty\"",
                            other
                        ))),
                    };
                return Ok(());
            }

            Err(meta.error("Unknown parameter. Supported: percentiles=[..], format=\"..\""))
        });

        if let Err(e) = parser.parse2(proc_macro2::TokenStream::from(attr)) {
            return e.to_compile_error().into();
        }
    }

    let percentiles_array = quote! { &[#(#percentiles),*] };
    let format_token = format.to_tokens();

    let output = quote! {
        #vis #sig {
            let _hotpath = {
                fn __caller_fn() {}
                let caller_name = std::any::type_name_of_val(&__caller_fn)
                    .strip_suffix("::__caller_fn")
                    .unwrap_or(std::any::type_name_of_val(&__caller_fn))
                    .replace("::{{closure}}", "");

                hotpath::HotPathBuilder::new(caller_name.to_string())
                    .percentiles(#percentiles_array)
                    .format(#format_token)
                    .build()
            };

            #block
        }
    };

    output.into()
}

#[proc_macro_attribute]
pub fn measure(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    let name = sig.ident.to_string();
    let asyncness = sig.asyncness.is_some();

    let output = if asyncness {
        quote! {
            #vis #sig {
                async {
                    hotpath::cfg_if! {
                        if #[cfg(feature = "hotpath-off")] {
                            // No-op when hotpath-off is enabled
                        } else if #[cfg(any(
                            feature = "hotpath-alloc-bytes-total",
                            feature = "hotpath-alloc-bytes-max",
                            feature = "hotpath-alloc-count-total",
                            feature = "hotpath-alloc-count-max"
                        ))] {
                            use hotpath::{Handle, RuntimeFlavor};
                            let runtime_flavor = Handle::try_current().ok().map(|h| h.runtime_flavor());

                            let _guard = match runtime_flavor {
                                Some(RuntimeFlavor::CurrentThread) => {
                                    hotpath::AllocGuardType::AllocGuard(hotpath::AllocGuard::new(concat!(module_path!(), "::", #name)))
                                }
                                _ => {
                                    hotpath::AllocGuardType::NoopAsyncAllocGuard(hotpath::NoopAsyncAllocGuard::new(concat!(module_path!(), "::", #name)))
                                }
                            };
                        } else {
                            let _guard = hotpath::TimeGuard::new(concat!(module_path!(), "::", #name));
                        }
                    }

                    #block
                }.await
            }
        }
    } else {
        quote! {
            #vis #sig {
                hotpath::cfg_if! {
                    if #[cfg(feature = "hotpath-off")] {
                        // No-op when hotpath-off is enabled
                    } else if #[cfg(any(
                        feature = "hotpath-alloc-bytes-total",
                        feature = "hotpath-alloc-bytes-max",
                        feature = "hotpath-alloc-count-total",
                        feature = "hotpath-alloc-count-max"
                    ))] {
                        let _guard = hotpath::AllocGuard::new(concat!(module_path!(), "::", #name));
                    } else {
                        let _guard = hotpath::TimeGuard::new(concat!(module_path!(), "::", #name));
                    }
                }

                #block
            }
        }
    };

    output.into()
}
