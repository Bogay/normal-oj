use axum::http::StatusCode;
use cookie::SameSite;
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    mailers::auth::AuthMailer,
    models::{
        _entities::courses,
        users::{self, LoginParams, RegisterParams, Role},
    },
    views::{auth::LoginResponse, user::UserInfoResponse, NojResponseBuilder},
};

use super::find_user_by_auth;

#[derive(Debug, Deserialize, Serialize)]
pub struct VerifyParams {
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ForgotParams {
    pub email: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResetParams {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChangePasswordParams {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CheckItemParams {
    pub username: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchSignupParams {
    new_users: String,
    course: Option<String>,
    force: Option<bool>,
}

/// Register function creates a new user with the given parameters and sends a
/// welcome email to the user
async fn register(
    State(ctx): State<AppContext>,
    Json(params): Json<RegisterParams>,
) -> Result<Response> {
    let res = users::Model::create_with_password(&ctx.db, &params).await;

    let user = match res {
        Ok(user) => user,
        Err(err) => {
            tracing::info!(
                message = err.to_string(),
                user_email = &params.email,
                "could not register user",
            );
            return format::json(());
        }
    };

    let user = user
        .into_active_model()
        .set_email_verification_sent(&ctx.db)
        .await?;

    AuthMailer::send_welcome(&ctx, &user).await?;

    format::json(())
}

/// Verify register user. if the user not verified his email, he can't login to
/// the system.
async fn verify(
    State(ctx): State<AppContext>,
    Json(params): Json<VerifyParams>,
) -> Result<Response> {
    let user = users::Model::find_by_verification_token(&ctx.db, &params.token).await?;

    if user.email_verified_at.is_some() {
        tracing::info!(pid = user.pid.to_string(), "user already verified");
    } else {
        let active_model = user.into_active_model();
        let user = active_model.verified(&ctx.db).await?;
        tracing::info!(pid = user.pid.to_string(), "user verified");
    }

    format::json(())
}

/// In case the user forgot his password  this endpoints generate a forgot token
/// and send email to the user. In case the email not found in our DB, we are
/// returning a valid request for for security reasons (not exposing users DB
/// list).
async fn forgot(
    State(ctx): State<AppContext>,
    Json(params): Json<ForgotParams>,
) -> Result<Response> {
    let Ok(user) = users::Model::find_by_email(&ctx.db, &params.email).await else {
        // we don't want to expose our users email. if the email is invalid we still
        // returning success to the caller
        return format::json(());
    };

    let user = user
        .into_active_model()
        .set_forgot_password_sent(&ctx.db)
        .await?;

    AuthMailer::forgot_password(&ctx, &user).await?;

    format::json(())
}

/// reset user password by the given parameters
async fn reset(State(ctx): State<AppContext>, Json(params): Json<ResetParams>) -> Result<Response> {
    let Ok(user) = users::Model::find_by_reset_token(&ctx.db, &params.token).await else {
        // we don't want to expose our users email. if the email is invalid we still
        // returning success to the caller
        tracing::info!("reset token not found");

        return format::json(());
    };
    user.into_active_model()
        .reset_password(&ctx.db, &params.password)
        .await?;

    format::json(())
}

/// Creates a user login and returns a token (also set cookie)
async fn login(State(ctx): State<AppContext>, Json(params): Json<LoginParams>) -> Result<Response> {
    let login_failed = || {
        format::render().status(StatusCode::UNAUTHORIZED).json(
            NojResponseBuilder::new(())
                .message("Login Failed".to_string())
                .done(),
        )
    };
    let query_result = match users::Model::find_by_email(&ctx.db, &params.username).await {
        Ok(u) => Ok(u),
        Err(_) => users::Model::find_by_username(&ctx.db, &params.username).await,
    }
    .map_err(|e| match e {
        ModelError::EntityNotFound => login_failed(),
        _ => Err(loco_rs::Error::Any(e.into())),
    });

    let user = match query_result {
        Ok(u) => u,
        Err(e) => return e,
    };

    let valid = user.verify_password(&params.password);

    if !valid {
        return login_failed();
    }

    let jwt_secret = ctx.config.get_jwt_config()?;

    let Ok(token) = user.generate_jwt(&jwt_secret.secret, &jwt_secret.expiration) else {
        return login_failed();
    };
    let cookie = {
        let mut c = cookie::Cookie::new("piann", &token);
        c.set_http_only(true);
        c.set_path("/");
        c.set_same_site(SameSite::Lax);
        #[allow(clippy::cast_possible_wrap)]
        c.set_expires(
            time::OffsetDateTime::now_utc() + time::Duration::seconds(jwt_secret.expiration as i64),
        );
        c
    };
    format::render()
        .cookies(&[cookie])?
        .json(LoginResponse::new(&user, &token))
}

async fn change_password(
    State(ctx): State<AppContext>,
    auth: auth::JWT,
    Json(params): Json<ChangePasswordParams>,
) -> Result<Response> {
    let user = users::Model::find_by_claims_key(&ctx.db, &auth.claims.pid).await?;

    if !user.verify_password(&params.old_password) {
        return unauthorized("Wrong Password");
    }

    user.into_active_model()
        .reset_password(&ctx.db, &params.new_password)
        .await?;

    format::json(json!({"msg": "Password Has Been Changed"}))
}

async fn check(
    State(ctx): State<AppContext>,
    Path(item): Path<String>,
    Json(params): Json<CheckItemParams>,
) -> Result<Response> {
    match item.as_str() {
        "username" => {
            let Some(username) = params.username else {
                return format::render()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .json(json!({"msg": "missing field 'username'"}));
            };

            match users::Model::find_by_username(&ctx.db, &username).await {
                Ok(_) => format::render()
                    .status(StatusCode::CONFLICT)
                    .json(json!({"valid": 1})),
                Err(ModelError::EntityNotFound) => format::json(json!({"valid": 1})),
                Err(_) => format::render()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .empty(),
            }
        }
        "email" => {
            let Some(email) = params.email else {
                return format::render()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .json(json!({"msg": "missing field 'username'"}));
            };

            match users::Model::find_by_email(&ctx.db, &email).await {
                Ok(_) => format::render()
                    .status(StatusCode::CONFLICT)
                    .json(json!({"valid": 1})),
                Err(ModelError::EntityNotFound) => format::json(json!({"valid": 1})),
                Err(_) => format::render()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .empty(),
            }
        }
        _ => format::render()
            .status(StatusCode::BAD_REQUEST)
            .json(json!({"msg": "Invalid Checking Type"})),
    }
}

async fn batch_signup(
    State(ctx): State<AppContext>,
    auth: auth::JWT,
    Json(params): Json<BatchSignupParams>,
) -> Result<Response> {
    let user = users::Model::find_by_pid(&ctx.db, &auth.claims.pid).await?;
    if Role::Admin != user.role {
        return format::render()
            .status(StatusCode::FORBIDDEN)
            .json(json!({"msg": "Insufficient Permissions"}));
    }

    let new_users = csv::Reader::from_reader(params.new_users.as_bytes())
        .deserialize()
        .map(|row| row.map_err(|e| loco_rs::Error::Any(Box::new(e))))
        .collect::<Result<Vec<users::BatchSignupItem>>>()
        .map_err(|_| {
            // TODO: this should be 422: Unprocessable Content?
            loco_rs::Error::BadRequest("Error parse csv file".to_string())
        })?;
    let course = match params.course {
        Some(c) => Some(courses::Model::find_by_name(&ctx.db, &c).await?),
        None => None,
    };

    let params = users::BatchSignupParams {
        course,
        users: new_users,
    };

    let new_users = users::Model::batch_signup(&ctx.db, &params).await?;
    tracing::info!(count = new_users.len(), "new users created");
    format::render().status(StatusCode::CREATED).empty()
}

async fn me(State(ctx): State<AppContext>, auth: auth::JWT) -> Result<Response> {
    let user = match find_user_by_auth(&ctx, &auth).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    format::json(NojResponseBuilder::new(UserInfoResponse::new(&user)).done())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("auth")
        .add("/register", post(register))
        .add("/verify", post(verify))
        .add("/login", post(login))
        .add("/session", post(login))
        .add("/forgot", post(forgot))
        .add("/reset", post(reset))
        .add("/change-password", post(change_password))
        .add("/check/:item", post(check))
        .add("/batch-signup", post(batch_signup))
        .add("/me", get(me))
}
