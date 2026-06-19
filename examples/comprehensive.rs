#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "lowercase")]
enum Role {
    Admin,
    Editor,
    Viewer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "snake_case")]
enum AccountStatus {
    PendingInvite,
    Active,
    Suspended,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "snake_case")]
enum SortOrder {
    CreatedAsc,
    CreatedDesc,
    NameAsc,
    NameDesc,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    BadRequest,
    NotFound,
    Conflict,
    ValidationFailed,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
struct UserProfile {
    display_name: Option<String>,
    bio: Option<String>,
    website: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NotificationSettings {
    email_enabled: bool,
    product_updates: bool,
    weekly_digest: bool,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "camelCase")]
struct User {
    id: String,
    email: String,
    name: String,
    role: Role,
    status: AccountStatus,
    tags: Vec<String>,
    metadata: BTreeMap<String, String>,
    profile: Option<UserProfile>,
    notification_settings: NotificationSettings,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateUser {
    email: String,
    name: String,
    role: Role,
    tags: Vec<String>,
    metadata: BTreeMap<String, String>,
    profile: Option<UserProfile>,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UpdateUser {
    email: Option<String>,
    name: Option<String>,
    role: Option<Role>,
    status: Option<AccountStatus>,
    tags: Option<Vec<String>>,
    metadata: Option<BTreeMap<String, String>>,
    profile: Option<UserProfile>,
    notification_settings: Option<NotificationSettings>,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "camelCase")]
struct UserList {
    items: Vec<User>,
    total: u64,
    page: u32,
    per_page: u32,
    has_next_page: bool,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
struct ValidationIssue {
    field: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize, ts_rs::TS, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ErrorResponse {
    code: ErrorCode,
    message: String,
    request_id: String,
    issues: Vec<ValidationIssue>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
struct ListUsersQuery {
    /// Free-text search over user name and email.
    q: Option<String>,
    /// Filter users by role.
    role: Option<Role>,
    /// Filter users by account status.
    status: Option<AccountStatus>,
    /// Sort order for the result list.
    sort: Option<SortOrder>,
    #[serde(default)]
    /// Include suspended users in the result list.
    include_suspended: bool,
    #[serde(default)]
    /// Require users to have at least one of these tags.
    tags: Vec<String>,
    #[serde(default)]
    /// Page number starting from 1.
    page: u32,
    #[serde(default)]
    /// Number of users to return per page.
    per_page: u32,
}

#[utoipa_ts::path(
    get,
    path = "/users",
    params(ListUsersQuery),
    responses(
        (
            status = 200,
            description = "Users matching the supplied filters",
            body = UserList,
            content_type = "application/json",
            headers(
                ("x-total-count" = String, description = "Total number of users matching the filters")
            )
        ),
        (status = 400, description = "Invalid query", body = ErrorResponse),
    )
)]
async fn list_users() {}

#[utoipa_ts::path(
    post,
    path = "/users",
    request_body(content = CreateUser, description = "User fields for a new account"),
    responses(
        (
            status = 201,
            description = "User created",
            body = User,
            content_type = "application/json",
            headers(("location" = String, description = "URL of the created user"))
        ),
        (status = 409, description = "Email already exists", body = ErrorResponse),
        (status = 422, description = "Validation failed", body = ErrorResponse),
    )
)]
async fn create_user() {}

#[utoipa_ts::path(
    get,
    path = "/users/{id}",
    params(
        ("id" = String, Path, description = "User ID"),
        (
            "x-request-id" = String,
            Header,
            description = "Request correlation ID supplied by the client"
        ),
        (
            "session_id" = String,
            Cookie,
            description = "Session cookie used for this request"
        ),
    ),
    responses(
        (status = 200, description = "User found", body = User),
        (status = 404, description = "User not found", body = ErrorResponse),
    )
)]
async fn get_user() {}

#[utoipa_ts::path(
    patch,
    path = "/users/{id}",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    request_body(content = UpdateUser, description = "Partial user update"),
    responses(
        (status = 200, description = "User updated", body = User),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 422, description = "Validation failed", body = ErrorResponse),
    )
)]
async fn update_user() {}

#[utoipa_ts::path(
    delete,
    path = "/users/{id}",
    params(
        ("id" = String, Path, description = "User ID"),
    ),
    responses(
        (status = 204, description = "User deleted"),
        (status = 404, description = "User not found", body = ErrorResponse),
    )
)]
async fn delete_user() {}

utoipa_ts::export!("examples/comprehensive.d.ts");

#[derive(OpenApi)]
#[openapi(
    paths(list_users, create_user, get_user, update_user, delete_user),
    components(schemas(
        AccountStatus,
        CreateUser,
        ErrorCode,
        ErrorResponse,
        NotificationSettings,
        Role,
        SortOrder,
        UpdateUser,
        User,
        UserList,
        UserProfile,
        ValidationIssue,
    ))
)]
struct ApiDoc;

fn main() {
    println!("{}", ApiDoc::openapi().to_pretty_json().unwrap());
}
