# [juniper-from-schema](https://crates.io/crates/juniper-from-schema)

![Build](https://github.com/davidpdrsn/juniper-from-schema/workflows/Build/badge.svg)
[![crates.io](https://meritbadge.herokuapp.com/juniper-from-schema)](https://crates.io/crates/juniper-from-schema)
[![Documentation](https://docs.rs/juniper-from-schema/badge.svg)](https://docs.rs/juniper-from-schema)

This library contains a procedural macro that reads a GraphQL schema file, and generates the
corresponding [Juniper](https://crates.io/crates/juniper) [macro calls]. This means you can
have a real schema file and be guaranteed that it matches your Rust implementation. It also
removes most of the boilerplate involved in using Juniper.

[macro calls]: https://graphql-rust.github.io/types/objects/complex_fields.html


# Looking for juniper 0.15 support?

The version of juniper-from-schema that is released on crates.io (0.5.2) doesn't support juniper 0.15.
However, the master branch does! So you will have to use a git dependency for now.
We plan to do an official release soon.
Follow [this](https://github.com/davidpdrsn/juniper-from-schema/milestone/1) milestone to see what's left.


# Example

Imagine you have a GraphQL schema like this:

```graphql
schema {
  query: Query
}

type Query {
  helloWorld(name: String!): String! @juniper(ownership: "owned")
}
```

That can be implemented like so:

```rust
use juniper_from_schema::graphql_schema_from_file;

// This is the important line
graphql_schema_from_file!("readme_schema.graphql");

pub struct Context;

impl juniper::Context for Context {}

pub struct Query;

// This trait is generated by `graphql_schema_from_file!` based on the schema
impl QueryFields for Query {
    fn field_hello_world(
        &self,
        _executor: &juniper::Executor<Context>,
        name: String,
    ) -> juniper::FieldResult<String> {
        Ok(format!("Hello, {}!", name))
    }
}

fn main() {
    let ctx = Context;

    let query = "query { helloWorld(name: \"Ferris\") }";

    let (result, errors) = juniper::execute_sync(
        query,
        None,
        &Schema::new(Query, juniper::EmptyMutation::new()),
        &juniper::Variables::new(),
        &ctx,
    )
    .unwrap();

    assert_eq!(errors.len(), 0);
    assert_eq!(
        result
            .as_object_value()
            .unwrap()
            .get_field_value("helloWorld")
            .unwrap()
            .as_scalar_value::<String>()
            .unwrap(),
        "Hello, Ferris!",
    );
}
```

See the [crate documentation](https://docs.rs/juniper-from-schema/) for a usage examples and more info.

# N+1s

If you're having issues with N+1 query bugs consider using [juniper-eager-loading](https://crates.io/crates/juniper-eager-loading). It was built to integrate seamlessly with juniper-from-schema.


# Development

## If you're seeing `No such file or directory (os error 2)` when running the tests

This might be caused by setting `CARGO_TARGET_DIR`. Setting that env var changes the directory the [trybuild](https://crates.io/crates/trybuild) tests are run from which means all the paths to the test schemas no longer match. The only workaround is to unset `CARGO_TARGET_DIR` when working on juniper-from-schema. I recommend [direnv](https://github.com/direnv/direnv) to unset the env var only this directory and not globally.
