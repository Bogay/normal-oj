use axum::body::Bytes;
use loco_rs::prelude::*;
use loco_rs::testing;
use normal_oj::app::App;
use normal_oj::models::problems;
use normal_oj::models::problems::Type;
use normal_oj::models::problems::Visibility;
use normal_oj::models::submissions;
use normal_oj::models::users;
use normal_oj::workers::submission::SubmissionWorker;
use normal_oj::workers::submission::SubmissionWorkerArgs;
use serial_test::serial;

use crate::make_test_case;

#[tokio::test]
#[serial]
async fn test_run_submission_worker_worker() {
    tracing::debug!("booting");
    let boot = testing::boot_test::<App>().await.unwrap();
    let ctx = &boot.app_context;
    let db = &ctx.db;
    testing::seed::<App>(db).await.unwrap();

    let first_admin = users::Model::find_by_username(&ctx.db, "first_admin")
        .await
        .unwrap();
    let problem = problems::Model::add(
        &ctx.db,
        &problems::AddParams {
            owner: first_admin.clone(),
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
                memory_limit: 536870912,
            }],
        },
    )
    .await
    .unwrap();

    let file_content = make_test_case(db, &problem).await.unwrap();

    let test_case_id = uuid::Uuid::new_v4();
    let problem = problem
        .into_active_model()
        .update_test_case_id(&ctx.db, Some(test_case_id.to_string()))
        .await
        .unwrap();

    // because we just set its test case id, it's safe to unwrap()
    let path = problem.test_case_path().unwrap();
    ctx.storage
        .as_ref()
        .upload(path.as_path(), &Bytes::from(file_content))
        .await
        .unwrap();

    let subm = submissions::Model::add(
        db,
        &submissions::AddParams {
            user: first_admin.id,
            problem: problem.id,
            timestamp: chrono::Utc::now().naive_utc(),
            language: submissions::Language::C,
        },
    )
    .await
    .unwrap();

    let subm = subm
        .into_active_model()
        .update_code(
            db,
            r#"#include <stdio.h>

int main()
{
        puts("hello world!");
        return 0;
}
"#
            .to_string(),
        )
        .await
        .unwrap();

    assert!(SubmissionWorker::perform_later(
        &boot.app_context,
        SubmissionWorkerArgs {
            submission_id: subm.id,
        }
    )
    .await
    .is_ok());

    let subm = subm.into_active_model().update(db).await.unwrap();
    assert_eq!(100, subm.score);
}
