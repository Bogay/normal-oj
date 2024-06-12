use crate::{
    models::{
        self,
        problems::{self, Type, Visibility},
        transform_db_error, users,
    },
    views::problems::{ProblemDetailResponse, ProblemListResponse},
};
use axum::extract::{DefaultBodyLimit, Multipart, Query};
use loco_rs::{controller::format::render, prelude::*};
use serde::Deserialize;
use std::path::PathBuf;

use super::{find_user_by_auth, permission_denied, verify_admin};

#[derive(Debug, Deserialize)]
pub struct CreateProblemRequest {
    /// list of course names
    pub courses: Vec<String>,
    pub name: String,
    /// Problem status, control its visibility
    pub status: Option<Visibility>,
    /// Problem description struct
    pub description: problems::descriptions::AddParams,
    pub r#type: Option<Type>,
    pub allowed_language: Option<i32>,
    pub quota: Option<i32>,
    pub tasks: Vec<problems::tasks::AddParams>,
}

async fn create(
    State(ctx): State<AppContext>,
    auth: auth::JWT,
    Json(params): Json<CreateProblemRequest>,
) -> Result<Response> {
    let user = match verify_admin(&ctx, &auth).await {
        Ok(u) => u,
        Err(e) => return e,
    };

    let params = problems::AddParams {
        owner: user,
        courses: params.courses,
        name: params.name,
        status: params.status,
        description: params.description,
        r#type: params.r#type,
        allowed_language: params.allowed_language,
        quota: params.quota,
        tasks: params.tasks,
    };

    let problem = problems::Model::add(&ctx.db, &params).await?;

    render().json(problem)
}

#[derive(Debug, Deserialize)]
pub struct ListProblemRequest {
    pub offset: Option<usize>,
    /// how many problems to return, -1 to return all
    pub count: Option<i32>,
    pub name: Option<String>,
    pub tags: Option<String>,
    pub course: Option<String>,
}

async fn list(
    State(ctx): State<AppContext>,
    auth: auth::JWT,
    params: Query<ListProblemRequest>,
) -> Result<Response> {
    let user = users::Model::find_by_claims_key(&ctx.db, &auth.claims.pid).await?;

    let params = problems::ListParams {
        viewer: user,
        offset: params.offset,
        count: params.count,
        name: params.name.clone(),
        tags: params
            .tags
            .as_ref()
            .map(|t| t.split(',').map(str::to_string).collect()),
        course: params.course.clone(),
    };

    let problems = problems::Model::list(&ctx.db, &params).await?;

    format::json(ProblemListResponse::new(&problems).done())
}

async fn get_problem(
    State(ctx): State<AppContext>,
    // auth: auth::JWT,
    Path(problem_id): Path<i32>,
) -> Result<Response> {
    // TOOD: authz

    let prob = problems::Model::find_by_id(&ctx.db, problem_id).await?;
    let desc = prob
        .find_related(models::_entities::problem_descriptions::Entity)
        .one(&ctx.db)
        .await
        .map_err(transform_db_error)?
        .ok_or(ModelError::EntityNotFound)?;
    let owner = prob
        .find_related(models::_entities::users::Entity)
        .one(&ctx.db)
        .await
        .map_err(transform_db_error)?
        .ok_or(ModelError::EntityNotFound)?;
    let tasks = prob.tasks(&ctx.db).await?;

    format::json(ProblemDetailResponse::new(&prob, &desc, &owner, &tasks).done())
}

async fn upload_test_case(
    State(ctx): State<AppContext>,
    auth: auth::JWT,
    Path(problem_id): Path<i32>,
    mut multipart: Multipart,
) -> Result<Response> {
    let user = match find_user_by_auth(&ctx, &auth).await {
        Ok(u) => u,
        Err(e) => return e,
    };
    let prob = problems::Model::find_by_id(&ctx.db, problem_id).await?;
    if user.id != prob.owner_id {
        return permission_denied();
    }

    let file_content = loop {
        let Some(field) = multipart.next_field().await.map_err(|err| {
            tracing::error!(error = ?err,"could not read multipart");
            Error::BadRequest("could not read multipart".into())
        })?
        else {
            return Err(Error::BadRequest("cloud not find test case file".into()));
        };

        if !matches!(field.content_type(), Some("application/x-zip")) {
            continue;
        }

        break field.bytes().await.map_err(|err| {
            tracing::error!(error = ?err,"could not read bytes");
            Error::BadRequest("could not read bytes".into())
        })?;
    };

    prob.validate_test_case(&ctx.db, &file_content).await?;
    tracing::info!(problem_id = prob.id, "test case validated");

    let test_case_id = uuid::Uuid::new_v4();
    let prob = prob
        .into_active_model()
        .update_test_case_id(&ctx.db, Some(test_case_id.to_string()))
        .await?;

    // because we just set its test case id, it's safe to unwrap()
    let path = prob.test_case_path().unwrap();
    ctx.storage
        .as_ref()
        .upload(path.as_path(), &file_content)
        .await?;
    tracing::info!(test_case_id = ?test_case_id, "test case uploaded");

    format::empty_json()
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("problems")
        .add("/", post(create))
        .add("/manage", post(create))
        .add("/", get(list))
        .add("/:problem_id", get(get_problem))
        .add("/view/:problem_id", get(get_problem))
        .add("/manage/:problem_id", put(upload_test_case))
        .add(
            "/:problem_id",
            // change body limit to 128 MB
            put(upload_test_case).layer(DefaultBodyLimit::max(128 * 1024 * 1024)),
        )
}
