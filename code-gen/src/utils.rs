use proc_macro2::TokenStream;

pub fn pretty_print(stream: &TokenStream) -> String {
    let file = syn::parse2(stream.clone())
        .expect("Couldn't parse token stream");

    prettyplease::unparse(&file)
}

