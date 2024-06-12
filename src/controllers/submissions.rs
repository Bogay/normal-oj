use axum::extract::Query;
use chrono::offset::Utc;
use chrono::DateTime;
use format::render;
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    models::{self, submissions, transform_db_error},
    views::submission::SubmissionListResponse,
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

    println!("{:?}", user);

    let params = submissions::AddParams {
        user: user.id,
        problem: params.problem_id,
        // timestamp: DateTime::from_str(&params.timestamp).map_err(Box::from)?,
        timestamp: DateTime::<Utc>::from_timestamp(params.timestamp, 0)
            .unwrap()
            .naive_utc(),
        language: params.language.into(),
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
        status: params.status.map(|s| s.into()),
        language: params.language.map(|l| l.into()),
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
pub struct UpdateSubmissionResultRequest {
    pub score: i32,
    pub exec_time: i32,
    pub memory_usage: i32,
}

async fn update_sandbox_result(
    State(ctx): State<AppContext>,
    Path(submission_id): Path<i32>,
    Json(params): Json<UpdateSubmissionResultRequest>,
) -> Result<Response> {
    let submission = submissions::Model::find_by_id(&ctx.db, submission_id).await?;
    submission
        .into_active_model()
        .update_sandbox_result(&ctx.db, params.score, params.exec_time, params.memory_usage)
        .await?;

    format::empty_json()
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("submissions")
        .add("/", get(list))
        .add("/", post(create))
        .add("/:submission_id", put(update_sandbox_result))
}
