use axum::http::StatusCode;
use insta::{assert_debug_snapshot, assert_json_snapshot, with_settings};
use loco_rs::testing;
use normal_oj::{app::App, models::users};
use rstest::rstest;
use serial_test::serial;

use crate::requests::create_cookie;

use super::prepare_data;

macro_rules! configure_insta {
    () => {
        crate::configure_insta!("auth_request");
    };
}

#[tokio::test]
#[serial]
async fn can_register() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let email = "test@loco.com";
        let payload = serde_json::json!({
            "username": "loco",
            "email": email,
            "password": "12341234"
        });

        let _response = request.post("/api/auth/register").json(&payload).await;
        let saved_user = users::Model::find_by_email(&ctx.db, email).await;

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_debug_snapshot!(saved_user);
        });

        with_settings!({
            filters => testing::cleanup_email()
        }, {
            assert_debug_snapshot!(ctx.mailer.unwrap().deliveries());
        });
    })
    .await;
}

#[rstest]
#[case("login_with_valid_password", "12341234", StatusCode::OK)]
#[case(
    "login_with_invalid_password",
    "invalid-password",
    StatusCode::UNAUTHORIZED
)]
#[tokio::test]
#[serial]
async fn can_login_with_verify(
    #[case] test_name: &str,
    #[case] password: &str,
    #[case] expected_code: StatusCode,
) {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let email = "test@loco.com";
        let register_payload = serde_json::json!({
            "username": "loco",
            "email": email,
            "password": "12341234"
        });

        //Creating a new user
        _ = request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;

        let user = users::Model::find_by_email(&ctx.db, email).await.unwrap();
        let verify_payload = serde_json::json!({
            "token": user.email_verification_token,
        });
        request.post("/api/auth/verify").json(&verify_payload).await;

        //verify user request
        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "username": email,
                "password": password
            }))
            .await;
        response.assert_status(expected_code);

        // Make sure email_verified_at is set
        assert!(users::Model::find_by_email(&ctx.db, email)
            .await
            .unwrap()
            .email_verified_at
            .is_some());

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_json_snapshot!(
                test_name,
                response.json::<serde_json::Value>(),
            );
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_login_without_verify() {
    configure_insta!();

    testing::request::<App, _, _>(|request, _ctx| async move {
        let email = "test@loco.com";
        let password = "12341234";
        let register_payload = serde_json::json!({
            "username": "loco",
            "email": email,
            "password": password
        });

        //Creating a new user
        _ = request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;

        //verify user request
        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "username": email,
                "password": password
            }))
            .await;

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
async fn can_reset_password() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let login_data = prepare_data::init_user_login(&request, &ctx).await;

        let forgot_payload = serde_json::json!({
            "email": login_data.user.email,
        });
        _ = request.post("/api/auth/forgot").json(&forgot_payload).await;

        let user = users::Model::find_by_email(&ctx.db, &login_data.user.email)
            .await
            .unwrap();
        assert!(user.reset_token.is_some());

        let new_password = "new-password";
        let reset_payload = serde_json::json!({
            "token": user.reset_token,
            "password": new_password,
        });

        let reset_response = request.post("/api/auth/reset").json(&reset_payload).await;

        let user = users::Model::find_by_email(&ctx.db, &user.email)
            .await
            .unwrap();
        assert!(user.reset_sent_at.is_some());

        assert_debug_snapshot!((reset_response.status_code(), reset_response.text()));

        let response = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "username": user.email,
                "password": new_password
            }))
            .await;

        assert_eq!(response.status_code(), 200);

        with_settings!({
            filters => testing::cleanup_email()
        }, {
            assert_debug_snapshot!(ctx.mailer.unwrap().deliveries());
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_change_password() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let login_data = prepare_data::init_user_login(&request, &ctx).await;
        let new_password = "here-is-a-new-password";

        let cookie = create_cookie(&login_data.token);
        let change_pass_payload = serde_json::json!({
            "old_password": login_data.password_plaintext,
            "new_password": new_password,
        });
        let resp = request
            .post("/api/auth/change-password")
            .json(&change_pass_payload)
            .add_cookie(cookie)
            .await;
        resp.assert_status_ok();

        // login with old password will fail
        let resp = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "username": login_data.user.email,
                "password": login_data.password_plaintext,
            }))
            .await;
        resp.assert_status_unauthorized();

        // login with new password
        let resp = request
            .post("/api/auth/login")
            .json(&serde_json::json!({
                "username": login_data.user.email,
                "password": new_password
            }))
            .await;
        resp.assert_status_ok();
    })
    .await;
}

#[rstest]
#[case("email", Some("user1@example.com"), 409)]
#[case("email", Some("user48763@example.com"), 200)]
#[case("username", Some("user1"), 409)]
#[case("username", Some("user48763"), 200)]
#[case("name", None, 400)]
#[case("mail", None, 400)]
#[tokio::test]
#[serial]
async fn can_check_identity_usage(
    #[case] item: &str,
    #[case] content: Option<&str>,
    #[case] expected_code: u16,
) {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let check_data = content.map(|c| {
            serde_json::json!({
                item: c,
            })
        });
        let resp = request
            .post(&format!("/api/auth/check/{item}"))
            .json(&check_data)
            .await;
        resp.assert_status(StatusCode::from_u16(expected_code).unwrap());
    })
    .await;
}
