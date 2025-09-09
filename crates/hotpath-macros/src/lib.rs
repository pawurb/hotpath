use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, ExprArray, ExprAssign, ExprPath, ItemFn, Lit, parse_macro_input};

#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    let percentiles = if attr.is_empty() {
        vec![95]
    } else {
        let attr_expr = parse_macro_input!(attr as Expr);
        match attr_expr {
            Expr::Assign(ExprAssign { left, right, .. }) => {
                if let Expr::Path(ExprPath { path, .. }) = left.as_ref() {
                    if path.is_ident("percentiles") {
                        if let Expr::Array(ExprArray { elems, .. }) = right.as_ref() {
                            parse_percentiles_array(elems)
                        } else {
                            panic!(
                                "Expected percentiles to be an array, e.g., percentiles = [50, 95, 99]"
                            )
                        }
                    } else {
                        panic!("Unknown parameter. Use: percentiles = [50, 95, 99]")
                    }
                } else {
                    panic!("Expected named parameter. Use: percentiles = [50, 95, 99]")
                }
            }
            _ => panic!(
                "Expected percentiles parameter with named syntax, e.g., percentiles = [50, 95, 99]"
            ),
        }
    };

    fn parse_percentiles_array(
        elems: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
    ) -> Vec<u8> {
        let mut parsed_percentiles = Vec::new();
        for elem in elems {
            if let Expr::Lit(lit_expr) = elem {
                if let Lit::Int(lit_int) = &lit_expr.lit {
                    let value: u8 = lit_int.base10_parse().unwrap_or_else(|_| {
                        panic!("Invalid percentile value: {}", lit_int.token())
                    });

                    // Validate percentile values at compile time (0-100)
                    if (0..=100).contains(&value) {
                        parsed_percentiles.push(value);
                    } else {
                        panic!(
                            "Invalid percentile: {}. Percentiles must be between 0 and 100.",
                            value
                        )
                    }
                } else {
                    panic!("Percentile values must be integer literals")
                }
            } else {
                panic!("Percentile values must be integer literals")
            }
        }
        if parsed_percentiles.is_empty() {
            panic!("At least one percentile must be specified")
        }
        parsed_percentiles
    }

    let percentiles_array = quote! { &[#(#percentiles),*] };

    let output = quote! {
        static __hotpath_main_guard: () = ();

        #vis #sig {
            #[cfg(feature = "hotpath")]
            let _hotpath = {
                fn __caller_fn() {}
                let caller_name = std::any::type_name_of_val(&__caller_fn);
                let caller_name = caller_name
                    .strip_suffix("::__caller_fn")
                    .unwrap_or(caller_name)
                    .replace("::{{closure}}", "")
                    .to_string();
                hotpath::init(caller_name, #percentiles_array)
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

    {
        let name = sig.ident.to_string();
        let asyncness = sig.asyncness.is_some();

        let output = if asyncness {
            quote! {
                #vis #sig {
                    async {
                        let _guard = hotpath::MeasureGuard::new(concat!(module_path!(), "::", #name));
                        #block
                    }.await
                }
            }
        } else {
            quote! {
                #vis #sig {
                    let _guard = hotpath::MeasureGuard::new(concat!(module_path!(), "::", #name));
                    #block
                }
            }
        };

        output.into()
    }
}
