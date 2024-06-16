use crate::requests::{create_cookie, create_token};

use super::prepare_data;
use loco_rs::{app::AppContext, testing};
use serde_json::json;
use serial_test::serial;

use normal_oj::{
    app::App,
    models::{
        problems::{self, Type, Visibility},
        users,
    },
};

macro_rules! configure_insta {
    () => {
        crate::configure_insta!("submission_request");
    };
}

async fn create_problem(ctx: &AppContext) -> problems::Model {
    let first_admin = users::Model::find_by_username(&ctx.db, "first_admin")
        .await
        .unwrap();

    problems::Model::add(
        &ctx.db,
        &problems::AddParams {
            owner: first_admin,
            courses: vec![],
            name: "test-course".to_string(),
            status: Some(Visibility::Show),
            description: problems::descriptions::AddParams {
                description: String::new(),
                input: String::new(),
                output: String::new(),
                hint: String::new(),
                sample_input: vec![],
                sample_output: vec![],
            },
            r#type: Some(Type::Normal),
            allowed_language: None,
            quota: None,
            tasks: vec![problems::tasks::AddParams {
                test_case_count: 2,
                score: 100,
                time_limit: 1000,
                memory_limit: 65535,
            }],
        },
    )
    .await
    .unwrap()
}

fn create_submission_payload(problem_id: i32) -> serde_json::Value {
    json!({
        "problemId": problem_id,
        "language": 0,
    })
}

#[tokio::test]
#[serial]
async fn create_submission() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let user = prepare_data::init_user_login(&request, &ctx).await;
        let problem = create_problem(&ctx).await;

        let cookie = create_cookie(&user.token);
        let response = request
            .post("/api/submissions")
            .add_cookie(cookie)
            .json(&create_submission_payload(problem.id))
            .await;
        response.assert_status_ok();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn upload_submission_code() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let user = prepare_data::init_user_login(&request, &ctx).await;
        let problem = create_problem(&ctx).await;

        let cookie = create_cookie(&user.token);
        let response = request
            .post("/api/submissions")
            .add_cookie(cookie)
            .json(&create_submission_payload(problem.id))
            .await;
        response.assert_status_ok();
        let submission_id = response
            .json::<serde_json::Value>()
            .as_object()
            .unwrap()
            .get("id")
            .unwrap()
            .as_i64()
            .unwrap();

        let code = r#"#include <stdio.h>
        int main()
        {
            int a, b;
            scanf("%d%d", &a, &b);
            printf("%d\n", a + b);

            return 0;
        }
        "#;

        let cookie = create_cookie(&user.token);
        let response = request
            .put(&format!("/api/submissions/{submission_id}"))
            .add_cookie(cookie)
            .json(&json!({
                "code": code,
            }))
            .await;
        response.assert_status_ok();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn get_nonexisting_submission_returns_404() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let first_admin = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let token = create_token(&first_admin, &ctx).await;
        let cookie = create_cookie(&token);

        let response = request
            .get("/api/submissions/12345")
            .add_cookie(cookie)
            .await;
        response.assert_status_not_found();
    })
    .await;
}
