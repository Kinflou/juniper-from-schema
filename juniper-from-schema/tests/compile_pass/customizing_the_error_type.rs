#![allow(dead_code, unused_variables, unused_must_use, unused_imports)]
include!("setup.rs");

juniper_from_schema::graphql_schema_from_file!(
    "../../../juniper-from-schema/tests/schemas/very_simple_schema.graphql",
    error_type: MyError,
);

pub enum MyError {
    Foo,
    Bar,
}

impl juniper::IntoFieldError for MyError {
    fn into_field_error(self) -> juniper::FieldError {
        unimplemented!()
    }
}

pub struct Query;

impl QueryFields for Query {
    fn field_string(&self, executor: &Executor<Context>) -> Result<&String, MyError> {
        unimplemented!()
    }
}
