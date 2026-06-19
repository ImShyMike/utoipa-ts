#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::RwLock;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

type Store = Arc<RwLock<BTreeMap<String, Todo>>>;

#[derive(Clone, Serialize, ts_rs::TS, ToSchema)]
struct Todo {
    id: String,
    title: String,
    done: bool,
}

#[derive(Deserialize, ts_rs::TS, ToSchema)]
struct CreateTodo {
    title: String,
}

#[derive(Deserialize, ts_rs::TS, ToSchema)]
struct UpdateTodo {
    title: Option<String>,
    done: Option<bool>,
}

#[utoipa_ts::path(
    get,
    path = "/todos",
    responses(
        (status = 200, description = "Todo list", body = Vec<Todo>),
    )
)]
async fn list_todos(State(store): State<Store>) -> Json<Vec<Todo>> {
    let todos = store.read().await.values().cloned().collect();
    Json(todos)
}

#[utoipa_ts::path(
    post,
    path = "/todos",
    request_body = CreateTodo,
    responses(
        (status = 201, description = "Todo created", body = Todo),
    )
)]
async fn create_todo(State(store): State<Store>, Json(input): Json<CreateTodo>) -> Json<Todo> {
    let mut todos = store.write().await;
    let id = (todos.len() + 1).to_string();
    let todo = Todo {
        id: id.clone(),
        title: input.title,
        done: false,
    };

    todos.insert(id, todo.clone());

    Json(todo)
}

#[utoipa_ts::path(
    put,
    path = "/todos/{id}",
    params(
        ("id" = String, Path, description = "Todo ID"),
    ),
    request_body(content = UpdateTodo, description = "Todo fields to update"),
    responses(
        (status = 200, description = "Todo updated", body = Todo),
        (status = 404, description = "Todo not found"),
    )
)]
async fn update_todo(
    State(store): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTodo>,
) -> Result<Json<Todo>, axum::http::StatusCode> {
    let mut todos = store.write().await;
    let Some(todo) = todos.get_mut(&id) else {
        return Err(axum::http::StatusCode::NOT_FOUND);
    };

    if let Some(title) = input.title {
        todo.title = title;
    }

    if let Some(done) = input.done {
        todo.done = done;
    }

    Ok(Json(todo.clone()))
}

utoipa_ts::export!("examples/axum.d.ts");

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store: Store = Arc::new(RwLock::new(BTreeMap::from([(
        "1".into(),
        Todo {
            id: "1".into(),
            title: "test todo".into(),
            done: false,
        },
    )])));

    let (router, _api) = OpenApiRouter::new()
        .routes(routes!(list_todos, create_todo))
        .routes(routes!(update_todo))
        .with_state(store)
        .split_for_parts();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("listening on http://{}", listener.local_addr()?);

    axum::serve(listener, router).await?;

    Ok(())
}
