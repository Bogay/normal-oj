use axum::http::StatusCode;
use insta::{assert_debug_snapshot, assert_json_snapshot, with_settings};
use loco_rs::testing;
use normal_oj::{
    app::App,
    models::users::{self, Role},
    views::{user::UserInfoResponse, PaginatedResponse},
};
use rstest::rstest;
use serde_json::json;
use serial_test::serial;

use crate::requests::create_cookie;

use super::{create_token, prepare_data};

macro_rules! configure_insta {
    () => {
        crate::configure_insta!("user_request");
    };
}

#[tokio::test]
#[serial]
async fn can_get_current_user() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let user = prepare_data::init_user_login(&request, &ctx).await;

        let cookie = create_cookie(&user.token);
        let response = request.get("/api/user/current").add_cookie(cookie).await;

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_debug_snapshot!((response.status_code(), response.text()));
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn normal_user_cannot_add_user() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let user = prepare_data::init_user_login(&request, &ctx).await;
        let create_user_payload = json!({
            "username": "new_user",
            "email": "somebody@noj.tw",
            "password": "password",
        });

        let cookie = create_cookie(&user.token);
        let response = request
            .post("/api/user")
            .json(&create_user_payload)
            .add_cookie(cookie)
            .await;

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_debug_snapshot!((response.status_code(), response.text()));
        });
    })
    .await;
}

#[rstest]
#[case("new_user@example.com", "new_user", StatusCode::CREATED)] // new user
#[case("new_user@example.com", "user1", StatusCode::CONFLICT)] // same name
#[case("user1@example.com", "new_user", StatusCode::CONFLICT)] // same email
#[case("user1@example.com", "user1", StatusCode::CONFLICT)] // duplicate user
#[case("first_admin@noj.tw", "first_admin", StatusCode::CONFLICT)] // duplicate admin
#[tokio::test]
#[serial]
async fn admin_can_add_user(
    #[case] email: &str,
    #[case] name: &str,
    #[case] status_code: StatusCode,
) {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let password = "password";

        let user = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let token = create_token(&user, &ctx).await;

        let create_user_payload: sea_orm::prelude::Json = json!({
            "username": name,
            "email": email,
            "password": password,
        });

        let cookie = create_cookie(&token);
        let add_user_response = request
            .post("/api/user")
            .json(&create_user_payload)
            .add_cookie(cookie)
            .await;

        add_user_response.assert_status(status_code);

        let try_login_response = request
            .post("/api/auth/login")
            .json(&json!({
                "username": email,
                "password": password,
            }))
            .await;

        with_settings!({
                filters => testing::cleanup_user_model(),
            }, {
                assert_json_snapshot!(
                    format!("admin_can_add_user_by_{email}_and_{name}"),
                    try_login_response.json::<serde_json::Value>(),
                    {
                        ".results" => insta::sorted_redaction()
                    },
                );
        });
    })
    .await;
}

#[rstest]
#[case("first_admin", StatusCode::OK)]
#[case("user1", StatusCode::FORBIDDEN)]
#[tokio::test]
#[serial]
async fn list_users(#[case] username: &str, #[case] status_code: StatusCode) {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let user = users::Model::find_by_username(&ctx.db, username)
            .await
            .unwrap();
        let token = create_token(&user, &ctx).await;
        let cookie = create_cookie(&token);
        let response = request.get("/api/user").add_cookie(cookie).await;
        response.assert_status(status_code);
        with_settings!({
            filters => testing::cleanup_user_model(),
        }, {
            assert_json_snapshot!(
                format!("list_users_{username}"),
                response.json::<serde_json::Value>(),
                {
                    ".results" => insta::sorted_redaction()
                },
            );
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_filter_user_by_role() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let user = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let token = create_token(&user, &ctx).await;
        let cookie = create_cookie(&token);
        let response = request
            .get("/api/user")
            .add_cookie(cookie)
            .add_query_param("role", Role::Admin as i32)
            .await;
        response.assert_status_ok();

        let response = response.json::<PaginatedResponse<UserInfoResponse>>();
        assert!(response
            .results
            .iter()
            .all(|u| Role::Admin as i32 == u.role));

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_json_snapshot!(response);
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_batch_signup() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();
        let payload = vec!["username,email,password".to_string()]
            .into_iter()
            .chain((3..6).map(|i| format!("user{i},user{i}@noj.tw,user{i}")))
            .collect::<Vec<_>>()
            .join("\n");
        let payload = json!({"new_users": payload});

        let user = users::Model::find_by_username(&ctx.db, "user1")
            .await
            .unwrap();
        let token = create_token(&user, &ctx).await;
        let cookie = create_cookie(&token);
        let response = request
            .post("/api/auth/batch-signup")
            .json(&payload)
            .add_cookie(cookie)
            .await;
        response.assert_status_forbidden();

        let user = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let token = create_token(&user, &ctx).await;
        let cookie = create_cookie(&token);
        let response = request
            .post("/api/auth/batch-signup")
            .add_cookie(cookie)
            .json(&payload)
            .await;
        response.assert_status_success();

        for i in 3..6 {
            let u = users::Model::find_by_username(&ctx.db, &format!("user{i}"))
                .await
                .unwrap();
            assert!(u.verify_password(&format!("user{i}")));
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn non_admin_cannot_edit_user() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();
        let user = users::Model::find_by_username(&ctx.db, "user1")
            .await
            .unwrap();
        let test_password = "random-test-password";
        let payload = json!({
            "password": test_password,
        });
        let token = create_token(&user, &ctx).await;
        let cookie = create_cookie(&token);
        let response = request
            .patch("/api/user/user2")
            .json(&payload)
            .add_cookie(cookie)
            .await;
        response.assert_status_forbidden();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn admin_can_edit_user() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();
        let user = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let test_password: &str = "random-test-password";
        let payload = json!({
            "password": test_password,
        });
        let token = create_token(&user, &ctx).await;
        let cookie = create_cookie(&token);
        let response = request
            .patch("/api/user/user2")
            .json(&payload)
            .add_cookie(cookie)
            .await;
        response.assert_status_ok();

        let user2 = users::Model::find_by_username(&ctx.db, "user2")
            .await
            .unwrap();
        assert!(user2.verify_password(test_password));
    })
    .await;
}
