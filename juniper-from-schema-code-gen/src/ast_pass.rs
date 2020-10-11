pub mod ast_data_pass;
pub mod code_gen_pass;
pub mod directive_parsing;
pub mod error;
pub mod schema_visitor;
pub mod validations;

pub use self::{code_gen_pass::CodeGenPass, error::ErrorKind};
use graphql_parser::Pos;

use graphql_parser::{query::Name, schema::Type};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{self, Ident};

pub fn ident<T: AsRef<str>>(name: T) -> Ident {
    Ident::new(name.as_ref(), Span::call_site())
}

// TODO: `Name` is a type alias for `String`. It would be nice if this returned a newtype so we
// would have more type safety. All this `&str` business makes me nervous.
pub fn type_name(type_: &Type) -> &Name {
    match &*type_ {
        Type::NamedType(name) => &name,
        Type::ListType(item_type) => type_name(&*item_type),
        Type::NonNullType(item_type) => type_name(&*item_type),
    }
}

pub fn quote_ident<T: AsRef<str>>(name: T) -> TokenStream {
    let ident = ident(name);
    quote! { #ident }
}

#[derive(Debug, Clone, Copy)]
pub enum TypeKind {
    Scalar,
    Type,
}

pub trait EmitError<'doc> {
    fn emit_non_fatal_error(&mut self, pos: Pos, kind: ErrorKind<'doc>);
}
