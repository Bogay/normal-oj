use num_traits::FromPrimitive;
use sea_orm::entity::prelude::DateTime;
use serde::Serialize;

use crate::models::{
    problems::{self, Type, Visibility},
    users,
};

use super::NojResponseBuilder;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemListResponseItem {
    pub id: i32,
    pub name: String,
    pub status: Visibility,
    #[serde(rename = "ACUser")]
    pub ac_user: i32,
    pub submitter: i32,
    pub tags: Vec<String>,
    pub r#type: Type,
    pub quota: i32,
    pub submit_count: i32,
}

pub struct ProblemListResponse {}

impl ProblemListResponse {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(problems: &[problems::Model]) -> NojResponseBuilder<Vec<ProblemListResponseItem>> {
        let data = problems
            .iter()
            .map(|p| ProblemListResponseItem {
                id: p.id,
                name: p.name.to_string(),
                status: Visibility::from_i32(p.status).unwrap(),
                r#type: Type::from_i32(p.r#type).unwrap(),
                quota: p.quota,
                // TODO: impl following fields
                ac_user: 0,
                submit_count: 0,
                submitter: 0,
                tags: vec![],
            })
            .collect();

        NojResponseBuilder::new(data)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDescriptionView {
    pub description: String,
    pub input: String,
    pub output: String,
    pub hint: String,
    pub sample_input: Vec<String>,
    pub sample_output: Vec<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemTaskView {
    pub test_case_count: i32,
    pub score: i32,
    pub time_limit: i32,
    pub memory_limit: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailResponse {
    problem_name: String,
    description: ProblemDescriptionView,
    /// username of problem owner
    owner: String,
    tags: Vec<String>,
    allowed_language: i32,
    /// list of courses' names
    courses: Vec<String>,
    quota: i32,
    status: Visibility,
    r#type: Type,
    test_case: Vec<ProblemTaskView>,
    submit_count: i32,
    high_score: i32,
}

impl ProblemDetailResponse {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        problem: &problems::Model,
        description: &problems::descriptions::Model,
        owner: &users::Model,
        tasks: &[problems::tasks::Model],
    ) -> NojResponseBuilder<Self> {
        let problems::descriptions::Model {
            description,
            input,
            output,
            hint,
            sample_input,
            sample_output,
            created_at,
            updated_at,
            ..
        } = description.clone();
        let to_task_view = |t: &problems::tasks::Model| {
            let problems::tasks::Model {
                test_case_count,
                score,
                time_limit,
                memory_limit,
                ..
            } = t.clone();

            ProblemTaskView {
                test_case_count,
                score,
                time_limit,
                memory_limit,
            }
        };

        let resp = Self {
            problem_name: problem.name.clone(),
            description: ProblemDescriptionView {
                description,
                input,
                output,
                hint,
                sample_input,
                sample_output,
                created_at,
                updated_at,
            },
            owner: owner.name.clone(),
            tags: vec![],
            allowed_language: problem.allowed_language,
            courses: vec![],
            quota: problem.quota,
            status: Visibility::from_i32(problem.status).unwrap(),
            r#type: Type::from_i32(problem.r#type).unwrap(),
            test_case: tasks.iter().map(to_task_view).collect(),
            submit_count: 0,
            high_score: 0,
        };
        NojResponseBuilder::new(resp)
    }
}
