use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

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
                        let __start = std::time::Instant::now();
                        let __result = { #block };
                        let __elapsed = __start.elapsed();

                        hotpath::send_measurement(concat!(module_path!(), "::", #name), __elapsed);

                        __result
                    }.await
                }
            }
        } else {
            quote! {
                #vis #sig {
                    let __start = std::time::Instant::now();
                    let __result = { #block };
                    let __elapsed = __start.elapsed();

                    hotpath::send_measurement(concat!(module_path!(), "::", #name), __elapsed);

                    __result
                }
            }
        };

        output.into()
    }
}
