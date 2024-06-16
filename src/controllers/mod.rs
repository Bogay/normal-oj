pub mod auth;
pub mod courses;
pub mod mock;
pub mod notes;
pub mod problems;
pub mod submissions;
pub mod user;

/// utils
use axum::http::StatusCode;
use loco_rs::{controller::middleware, prelude::*};
use serde_json::json;

use crate::models::users::{self, Role};

async fn verify_admin(
    ctx: &AppContext,
    auth: &middleware::auth::JWT,
) -> Result<users::Model, Result<Response>> {
    let user = find_user_by_auth(ctx, auth).await?;

    if Role::Admin != user.role {
        return Err(permission_denied());
    }

    Ok(user)
}

fn permission_denied() -> Result<Response> {
    format::render()
        .status(StatusCode::FORBIDDEN)
        .json(json!({"msg": "Insufficient Permissions"}))
}

async fn find_user_by_auth(
    ctx: &AppContext,
    auth: &loco_rs::prelude::auth::JWT,
) -> Result<users::Model, Result<Response>> {
    let user = users::Model::find_by_claims_key(&ctx.db, &auth.claims.pid)
        .await
        .map_err(|e| Err(e.into()))?;
    Ok(user)
}
