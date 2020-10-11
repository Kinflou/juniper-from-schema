#![allow(dead_code, unused_variables, unused_must_use, unused_imports)]
include!("setup.rs");

juniper_from_schema::graphql_schema_from_file!(
    "../../../juniper-from-schema/tests/schemas/returning_references.graphql"
);

pub struct Query;

impl QueryFields for Query {
    fn field_user_nullable(
        &self,
        executor: &Executor<Context>,
        trail: &QueryTrail<User, Walked>,
        id: i32,
    ) -> FieldResult<Option<User>> {
        Ok(find_user(id))
    }

    fn field_user_non_null(
        &self,
        executor: &Executor<Context>,
        trail: &QueryTrail<User, Walked>,
        id: i32,
    ) -> FieldResult<User> {
        Ok(find_user(id).unwrap())
    }
}

pub struct User {
    id: i32,
    name: String,
    name_nullable: Option<String>,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<Context>) -> FieldResult<&i32> {
        Ok(&self.id)
    }

    fn field_name_nullable(
        &self,
        executor: &Executor<Context>,
    ) -> FieldResult<Option<String>> {
        Ok(self.name_nullable.clone())
    }

    fn field_name_non_null(&self, executor: &Executor<Context>) -> FieldResult<&String> {
        Ok(&self.name)
    }
}

fn find_user(id: i32) -> Option<User> {
    Some(User {
        id,
        name: "Bob".to_string(),
        name_nullable: None,
    })
}
