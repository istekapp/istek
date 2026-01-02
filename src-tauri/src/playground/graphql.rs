use async_graphql::{Context, EmptySubscription, Object, Schema, SimpleObject, ID};
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::data::PlaygroundData;

/// GraphQL User type
#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlUser {
    pub id: ID,
    pub name: String,
    pub email: String,
    pub created_at: String,
}

impl From<&super::data::User> for GqlUser {
    fn from(user: &super::data::User) -> Self {
        Self {
            id: ID(user.id.to_string()),
            name: user.name.clone(),
            email: user.email.clone(),
            created_at: user.created_at.clone(),
        }
    }
}

/// GraphQL Query root
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Get all users
    async fn users(&self, ctx: &Context<'_>) -> Vec<GqlUser> {
        let data = ctx.data_unchecked::<Arc<PlaygroundData>>();
        let users = data.users.read().await;
        users.iter().map(GqlUser::from).collect()
    }

    /// Get a user by ID
    async fn user(&self, ctx: &Context<'_>, id: ID) -> Option<GqlUser> {
        let data = ctx.data_unchecked::<Arc<PlaygroundData>>();
        let users = data.users.read().await;
        let id_num: u32 = id.parse().ok()?;
        users.iter().find(|u| u.id == id_num).map(GqlUser::from)
    }
}

/// GraphQL Mutation root
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Create a new user
    async fn create_user(
        &self,
        ctx: &Context<'_>,
        name: String,
        email: String,
    ) -> GqlUser {
        let data = ctx.data_unchecked::<Arc<PlaygroundData>>();
        let new_user = super::data::User {
            id: data.next_user_id(),
            name,
            email,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let gql_user = GqlUser::from(&new_user);
        data.users.write().await.push(new_user);
        gql_user
    }

    /// Delete a user by ID
    async fn delete_user(&self, ctx: &Context<'_>, id: ID) -> bool {
        let data = ctx.data_unchecked::<Arc<PlaygroundData>>();
        let id_num: u32 = match id.parse() {
            Ok(n) => n,
            Err(_) => return false,
        };

        let mut users = data.users.write().await;
        let initial_len = users.len();
        users.retain(|u| u.id != id_num);
        users.len() < initial_len
    }

    /// Update a user
    async fn update_user(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: Option<String>,
        email: Option<String>,
    ) -> Option<GqlUser> {
        let data = ctx.data_unchecked::<Arc<PlaygroundData>>();
        let id_num: u32 = id.parse().ok()?;

        let mut users = data.users.write().await;
        let user = users.iter_mut().find(|u| u.id == id_num)?;

        if let Some(name) = name {
            user.name = name;
        }
        if let Some(email) = email {
            user.email = email;
        }

        Some(GqlUser::from(&*user))
    }
}

/// Create the GraphQL schema
pub type PlaygroundSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub fn create_schema(data: Arc<PlaygroundData>) -> PlaygroundSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(data)
        .finish()
}

/// GraphQL request body
#[derive(Debug, Deserialize)]
pub struct GraphQLRequestBody {
    pub query: String,
    #[serde(default)]
    pub variables: Option<serde_json::Value>,
    #[serde(default, rename = "operationName")]
    pub operation_name: Option<String>,
}

/// GraphQL response
#[derive(Debug, Serialize)]
pub struct GraphQLResponseBody {
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize)]
pub struct GraphQLError {
    pub message: String,
}

/// GraphQL handler - manual implementation to avoid version conflicts
async fn graphql_handler(
    State(schema): State<PlaygroundSchema>,
    Json(req): Json<GraphQLRequestBody>,
) -> Response {
    let mut request = async_graphql::Request::new(req.query);
    
    if let Some(vars) = req.variables {
        let variables = async_graphql::Variables::from_json(vars);
        request = request.variables(variables);
    }
    
    if let Some(op_name) = req.operation_name {
        request = request.operation_name(op_name);
    }

    let response = schema.execute(request).await;
    
    let body = GraphQLResponseBody {
        data: Some(serde_json::to_value(&response.data).unwrap_or(serde_json::Value::Null)),
        errors: if response.errors.is_empty() {
            None
        } else {
            Some(response.errors.iter().map(|e| GraphQLError {
                message: e.message.clone(),
            }).collect())
        },
    };

    Json(body).into_response()
}

/// GraphQL SDL (Schema Definition Language) endpoint
async fn graphql_sdl(State(schema): State<PlaygroundSchema>) -> String {
    schema.sdl()
}

/// Create the GraphQL router
pub fn create_graphql_router(data: Arc<PlaygroundData>) -> Router<Arc<PlaygroundData>> {
    let schema = create_schema(data);

    Router::new()
        .route("/graphql", post(graphql_handler).get(graphql_handler))
        .route("/graphql/sdl", get(graphql_sdl))
        .with_state(schema)
}
