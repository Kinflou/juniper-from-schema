
pub fn pretty_print(stream: &proc_macro2::TokenStream) -> String {
    let file = syn::parse2(stream.clone())
        .expect("Couldn't parse token stream");

    prettyplease::unparse(&file)
}
