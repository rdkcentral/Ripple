extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Nothing, Result};
use syn::{parse_quote, FnArg, ItemFn, PatType, ReturnType};

//use main::*;

/*
Given a fn f(...) wrap f in g such that g starts a timer, calls f, stops the timer, and then passes
it onto something else to render , transmit, etc. the value of the timer
*/
/*
hat tip: https://github.com/dtolnay/no-panic/blob/master/src/lib.rs
*/
#[proc_macro_attribute]
//  args are...args, and input is the function to be decorated
pub fn timed(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = TokenStream2::from(input);
    TokenStream::from(input)
}

#[proc_macro_attribute]
// attr is args to macro, item is the function to be decorated
pub fn counted(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = TokenStream2::from(args);
    let input = TokenStream2::from(input);
    let expanded = match parse(args, input.clone()) {
        Ok(function) => wrap_counted(function),
        Err(parse_error) => {
            let compile_error = parse_error.to_compile_error();
            quote!(#compile_error #input)
        }
    };
    TokenStream::from(expanded)
}

fn parse(args: TokenStream2, input: TokenStream2) -> Result<ItemFn> {
    let function: ItemFn = syn::parse2(input)?;
    let _: Nothing = syn::parse2::<Nothing>(args)?;

    Ok(function)
}
//write a function that takes a function and returns a f
fn wrap_counted(function: ItemFn) -> TokenStream2 {
    let mut wrapper = function.clone();
    /*
    determine if function is async  or not
    */
    let is_async = function.sig.asyncness.is_some();
    //extract the return type of the function
    let return_type = match &function.sig.output {
        ReturnType::Default => quote!(-> ()),
        ReturnType::Type(arrow, output) => {
            quote!(#arrow #output.clone())
        }
    };
    //extract the name of the function
    let function_name = &function.sig.ident;
    //extract the arguments of the function
    let mut args = Vec::new();
    for (_i, input) in wrapper.sig.inputs.iter_mut().enumerate() {
        let arg: &FnArg = input;
        match arg {
            FnArg::Typed(PatType {
                attrs: _,
                pat,
                colon_token: _,
                ty,
            }) => {
                let arg_name = quote!(#pat: #ty);
                // let arg_type = quote!(#ty);
                args.push(arg_name);
            }
            FnArg::Receiver(_) => {
                //do nothing
            }
        }
    }

    let original_block = function.block.clone();
    //println!("original block: {:?}",quote!{#original_block});
    let mut mutant = function.block.clone();
    mutant.stmts.clear();
    mutant.stmts.insert(0,parse_quote!({
        let mut timer = ripple_sdk::api::firebolt::fb_metrics::Timer::start(String::from("atimer"), None);
        let result = #original_block;
        timer.stop();        
        result
    }));

    let wrapper = if is_async {
        quote! {
            pub async  fn #function_name(#(#args),*) #return_type
                #mutant
        }
    } else {
        quote! {
            pub fn #function_name(#(#args),*) #return_type
                #mutant
        }
    };
    println!("wrapper: \n {}", wrapper);

    wrapper
}

// fn _enclose_with_counted(mut function: ItemFn) -> TokenStream2 {
//     let mut move_self = None;
//     let mut arg_pat = Vec::new();
//     let mut arg_val = Vec::new();
//     for (i, input) in function.sig.inputs.iter_mut().enumerate() {
//         let numbered = Ident::new(&format!("__arg{}", i), Span::call_site());
//         match input {
//             FnArg::Typed(PatType { pat, .. })
//                 if match pat.as_ref() {
//                     Pat::Ident(pat) => pat.ident != "self",
//                     _ => true,
//                 } =>
//             {
//                 arg_pat.push(quote!(#pat));
//                 arg_val.push(quote!(#numbered));
//                 *pat = parse_quote!(mut #numbered);
//             }
//             FnArg::Typed(_) | FnArg::Receiver(_) => {
//                 move_self = Some(quote! {
//                     if false {
//                         loop {}
//                         #[allow(unreachable_code)]
//                         {
//                             let __self = self;
//                         }
//                     }
//                 });
//             }
//         }
//     }

//     let has_inline = function
//         .attrs
//         .iter()
//         .any(|attr| attr.path().is_ident("inline"));
//     if !has_inline {
//         function.attrs.push(parse_quote!(#[inline]));
//     }

//     let ret = match &function.sig.output {
//         ReturnType::Default => quote!(-> ()),
//         ReturnType::Type(arrow, output) => {
//             let mut output = output.clone();
//             //make_impl_trait_wild(&mut output);
//             quote!(#arrow #output)
//         }
//     };
//     let stmts = function.block.stmts;
//     let message = format!(
//         "\n\nERROR[no-panic]: detected panic in function `{}`\n",
//         function.sig.ident,
//     );

//     function.block = Box::new(parse_quote!({
//         let mut timer = std::timer::
//     }));

//     quote!(#function)
// }
