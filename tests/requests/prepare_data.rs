use axum::http::{HeaderName, HeaderValue};
use loco_rs::{app::AppContext, TestServer};
use normal_oj::{models::users, views::auth::LoginResponse};

const USER_EMAIL: &str = "test@loco.com";
const USER_PASSWORD: &str = "1234";

pub struct LoggedInUser {
    pub user: users::Model,
    pub token: String,
    /// The unhashed password, stored here because we can't get that from user model
    pub password_plaintext: String,
}

/// Register an user and verify it.
pub async fn init_user_login(request: &TestServer, ctx: &AppContext) -> LoggedInUser {
    let register_payload = serde_json::json!({
        "username": "loco",
        "email": USER_EMAIL,
        "password": USER_PASSWORD
    });

    //Creating a new user
    request
        .post("/api/auth/register")
        .json(&register_payload)
        .await;
    let user = users::Model::find_by_email(&ctx.db, USER_EMAIL)
        .await
        .unwrap();

    let verify_payload = serde_json::json!({
        "token": user.email_verification_token,
    });

    request.post("/api/auth/verify").json(&verify_payload).await;

    let response = request
        .post("/api/auth/login")
        .json(&serde_json::json!({
            "username": USER_EMAIL,
            "password": USER_PASSWORD
        }))
        .await;

    let login_response: LoginResponse = serde_json::from_str(&response.text()).unwrap();

    LoggedInUser {
        user: users::Model::find_by_email(&ctx.db, USER_EMAIL)
            .await
            .unwrap(),
        token: login_response.token,
        password_plaintext: USER_PASSWORD.to_string(),
    }
}

pub fn auth_header(token: &str) -> (HeaderName, HeaderValue) {
    let auth_header_value = HeaderValue::from_str(&format!("Bearer {}", &token)).unwrap();

    (HeaderName::from_static("authorization"), auth_header_value)
}
