use syn::__private::ToTokens;

pub fn pretty_tokens(tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let file = syn::parse2(tokens)
        .expect("");

    let pretty = prettyplease::unparse(&file);

    syn::parse_file(&*pretty).unwrap().into_token_stream().into()
}