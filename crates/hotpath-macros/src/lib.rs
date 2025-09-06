use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    let output = quote! {
        static __hotpath_main_guard: () = ();

        #vis #sig {
            #[cfg(feature = "hotpath")]
            let _hotpath = hotpath::init!();

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
