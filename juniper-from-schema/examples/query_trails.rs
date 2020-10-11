#![allow(clippy::redundant_pattern_matching)]
#![allow(dead_code, unused_variables, unused_imports)]

use juniper::*;
use juniper_from_schema::graphql_schema;

fn main() {}

pub struct Context;

impl juniper::Context for Context {}

graphql_schema! {
    schema {
        query: Query
    }

    type Query {
        allPosts: [Post!]! @juniper(ownership: "owned")
    }

    type Post {
        id: Int!
        author: User!
    }

    type User {
        id: Int!
    }
}

pub struct Query;

impl QueryFields for Query {
    fn field_all_posts(
        &self,
        executor: &Executor<Context>,
        trail: &QueryTrail<Post, Walked>,
    ) -> FieldResult<Vec<Post>> {
        // Check if the query includes the author
        if let Some(_) = trail.author().walk() {
            // Somehow preload the users to avoid N+1 query bugs
            // Exactly how to do this depends on your setup
        }

        // Normally this would come from the database
        let post = Post {
            id: 1,
            author: User { id: 1 },
        };

        Ok(vec![post])
    }
}

pub struct Post {
    id: i32,
    author: User,
}

impl PostFields for Post {
    fn field_id(&self, executor: &Executor<Context>) -> FieldResult<&i32> {
        Ok(&self.id)
    }

    fn field_author(
        &self,
        executor: &Executor<Context>,
        trail: &QueryTrail<User, Walked>,
    ) -> FieldResult<&User> {
        Ok(&self.author)
    }
}

pub struct User {
    id: i32,
}

impl UserFields for User {
    fn field_id(&self, executor: &Executor<Context>) -> FieldResult<&i32> {
        Ok(&self.id)
    }
}
