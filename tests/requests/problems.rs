use std::{io::Write, path::Path};

use axum_test::multipart::{MultipartForm, Part};
use insta::{assert_debug_snapshot, with_settings};
use loco_rs::testing;
use normal_oj::{
    app::App,
    models::problems::{self, Type, Visibility},
    models::users,
};
use sea_orm::ConnectionTrait;
use serde_json::json;
use serial_test::serial;
use zip::write::SimpleFileOptions;

use crate::make_test_case;

use super::{create_token, prepare_data};

macro_rules! configure_insta {
    () => {
        crate::configure_insta!("problem_request");
    };
}

fn create_problem_payload() -> serde_json::Value {
    json!({
        "courses": [],
        "name": "A + B",
        "status": 1,
        "description": {
            "description": "",
            "input": "two space-separated number as A & B",
            "output": "A + B",
            "hint": "use +",
            "sample_input": ["1 2"],
            "sample_output": ["3"],
        },
        "tasks": [{
            "test_case_count": 2,
            "score": 100,
            "time_limit": 1000,
            "memory_limit": 65535,
        }]
    })
}

#[tokio::test]
#[serial]
async fn student_cannot_create_problem() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        let user = prepare_data::init_user_login(&request, &ctx).await;

        let (auth_key, auth_value) = prepare_data::auth_header(&user.token);
        let response = request
            .post("/api/problems")
            .add_header(auth_key, auth_value)
            .json(&create_problem_payload())
            .await;
        response.assert_status_forbidden();

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_debug_snapshot!(response.json::<serde_json::Value>());
        });
    })
    .await;
}

#[tokio::test]
#[serial]
async fn admin_can_create_problem_and_test_case() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let first_admin = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let (auth_key, auth_value) =
            prepare_data::auth_header(&create_token(&first_admin, &ctx).await);
        let problem = problems::Model::add(
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
        .unwrap();
        let test_case_content = make_test_case(&ctx.db, &problem).await.unwrap();
        let test_case = Part::bytes(test_case_content.clone())
            .file_name("test-case.zip")
            .mime_type("application/x-zip");
        let form = MultipartForm::new().add_part("case", test_case);
        let response = request
            .put(&format!("/api/problems/{}", problem.id))
            .add_header(auth_key, auth_value)
            .multipart(form)
            .await;
        response.assert_status_ok();

        let problem = problems::Model::find_by_id(&ctx.db, problem.id)
            .await
            .unwrap();

        let raw_path = format!("test-case/{}.zip", problem.test_case_id.unwrap());
        let path = Path::new(&raw_path);
        let uploaded_test_case: Vec<u8> = ctx.storage.download(path).await.unwrap();
        assert_eq!(test_case_content, uploaded_test_case);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn view_single_problem() {
    configure_insta!();

    testing::request::<App, _, _>(|request, ctx| async move {
        testing::seed::<App>(&ctx.db).await.unwrap();

        let user = prepare_data::init_user_login(&request, &ctx).await;
        let first_admin = users::Model::find_by_username(&ctx.db, "first_admin")
            .await
            .unwrap();
        let (auth_key, auth_value) = prepare_data::auth_header(&user.token);
        let problem = problems::Model::add(
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
        .unwrap();
        let response = request
            .get(&format!("/api/problems/{}", problem.id))
            .add_header(auth_key, auth_value)
            .await;
        response.assert_status_ok();

        with_settings!({
            filters => testing::cleanup_user_model()
        }, {
            assert_debug_snapshot!(response.json::<serde_json::Value>());
        });
    })
    .await;
}
