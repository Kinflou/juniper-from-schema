#![allow(dead_code, unused_variables, unused_must_use, unused_imports)]
include!("../compile_pass/setup.rs");

juniper_from_schema::graphql_schema! {
    type Query {
        string: String!
    }

    schema { query: Query }

    directive @juniper(
        ownership: Boolean,
        infallible: String = "foo",
        with_time_zone: [String] = false,
        bar: [Boolean]
    ) on FIELD
}

pub struct Query;

impl QueryFields for Query {
    fn field_string<'a>(&self, executor: &Executor<'a, Context>) -> FieldResult<&String> {
        unimplemented!()
    }
}
