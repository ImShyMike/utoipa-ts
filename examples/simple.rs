#![allow(dead_code)]

use utoipa::ToSchema;

#[derive(ts_rs::TS, ToSchema)]
struct User {
    id: String,
    name: String,
}

#[derive(ts_rs::TS, ToSchema)]
struct UpdateUser {
    name: String,
}

#[derive(ts_rs::TS, ToSchema)]
struct Todo {
    id: String,
    title: String,
    done: bool,
}

#[derive(ts_rs::TS, ToSchema)]
struct CreateTodo {
    title: String,
}

#[utoipa_ts::path(
    get,
    path = "/todos",
    responses(
        (status = 200, description = "Todo list", body = Vec<Todo>),
    )
)]
async fn list_todos() {}

#[utoipa_ts::path(
    post,
    path = "/todos",
    request_body = CreateTodo,
    responses(
        (status = 201, description = "Todo created", body = Todo),
    )
)]
async fn create_todo() {}

#[utoipa_ts::path(
    get,
    path = "/users/{id}",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 200, description = "User found", body = User),
        (status = 404, description = "User not found"),
    )
)]
async fn get_user() {}

#[utoipa_ts::path(
    put,
    path = "/users/{id}",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    request_body(content = UpdateUser, description = "Updated user fields"),
    responses(
        (status = 200, description = "User updated", body = User),
        (status = 404, description = "User not found"),
    )
)]
async fn update_user() {}

utoipa_ts::export!("types/simple"); // same as "types/simple/api.ts"

fn main() {}
