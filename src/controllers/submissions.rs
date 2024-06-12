use axum::{extract::Query, http::StatusCode};
use chrono::offset::Utc;
use chrono::DateTime;
use format::render;
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    models::{self, submissions, transform_db_error},
    views::submission::SubmissionListResponse,
    workers::submission::{SubmissionWorker, SubmissionWorkerArgs},
};

use super::find_user_by_auth;

#[derive(Debug, Deserialize)]
pub struct CreateSubmissionRequest {
    pub problem_id: i32,
    pub timestamp: i64,
    pub language: i32,
}

async fn create(
    State(ctx): State<AppContext>,
    auth: auth::JWT,
    Json(params): Json<CreateSubmissionRequest>,
) -> Result<Response> {
    let user = match find_user_by_auth(&ctx, &auth).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    let params = submissions::AddParams {
        user: user.id,
        problem: params.problem_id,
        timestamp: DateTime::<Utc>::from_timestamp(params.timestamp, 0)
            .unwrap()
            .naive_utc(),
        language: params.language.try_into().unwrap(),
    };

    let submission = submissions::Model::add(&ctx.db, &params).await?;

    render().json(submission)
}

#[derive(Debug, Deserialize)]
pub struct ListSubmissionRequest {
    pub offset: Option<usize>,
    pub count: Option<usize>,
    pub problem: Option<i32>,
    pub user: Option<i32>,
    pub status: Option<i32>,
    pub language: Option<i32>,
    pub course: Option<String>,
}

async fn list(
    State(ctx): State<AppContext>,
    params: Query<ListSubmissionRequest>,
) -> Result<Response> {
    let params = submissions::ListParams {
        offset: params.offset,
        count: params.count,
        problem: params.problem,
        user: params.user,
        status: params.status.map(|s| s.try_into().unwrap()),
        language: params.language.map(|l| l.try_into().unwrap()),
        course: params.course.clone(),
    };

    let submissions = submissions::Model::list(&ctx.db, &params).await?;
    let mut users = vec![];

    for s in &submissions {
        let u = s
            .find_related(models::_entities::users::Entity)
            .one(&ctx.db)
            .await
            .map_err(transform_db_error)?
            .ok_or(ModelError::EntityNotFound)?;
        users.push(u);
    }

    format::json(SubmissionListResponse::new(&submissions, &users).done())
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubmissionRequest {
    pub code: String,
}

async fn upload_code(
    State(ctx): State<AppContext>,
    Path(submission_id): Path<i32>,
    Json(params): Json<UpdateSubmissionRequest>,
) -> Result<Response> {
    let submission = submissions::Model::find_by_id(&ctx.db, submission_id).await?;
    let submission = submission
        .into_active_model()
        .update_code(&ctx.db, params.code)
        .await?;

    if let Err(e) = SubmissionWorker::perform_later(
        &ctx,
        SubmissionWorkerArgs {
            submission_id: submission.id,
        },
    )
    .await
    {
        tracing::error!(err = ?e, "failed to created submission work");
        return format::render()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .empty();
    }

    format::empty_json()
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("submissions")
        .add("/", get(list))
        .add("/", post(create))
        .add("/:submission_id", put(upload_code))
}
