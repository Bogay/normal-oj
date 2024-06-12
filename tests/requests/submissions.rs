use super::prepare_data;
use loco_rs::{app::AppContext, testing};
use serde_json::json;
use serial_test::serial;

use normal_oj::{
    app::App,
    models::{
        problems::{self, Type, Visibility},
        users::users,
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
    let ts = chrono::offset::Utc::now().timestamp();
    json!({
        "problem_id": problem_id,
        "timestamp": ts,
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

        let (auth_key, auth_value) = prepare_data::auth_header(&user.token);
        let response = request
            .post("/api/submissions")
            .add_header(auth_key, auth_value)
            .json(&create_submission_payload(problem.id))
            .await;
        response.assert_status_ok();
    })
    .await;
}
