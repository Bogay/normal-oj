use loco_rs::prelude::*;
use sea_orm::{Order, QueryOrder};
use serde::{Deserialize, Serialize};

use super::_entities::prelude::Submissions;
use super::_entities::problems;
pub use super::_entities::sea_orm_active_enums::{Language, SubmissionStatus};
pub use super::_entities::submissions::{self, ActiveModel, Model};

#[derive(Debug, Deserialize)]
pub struct AddParams {
    pub user: i32,
    pub problem: i32,
    pub timestamp: DateTime,
    pub language: Language,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub offset: Option<usize>,
    pub count: Option<usize>,
    pub problem: Option<i32>,
    pub user: Option<i32>,
    pub status: Option<SubmissionStatus>,
    pub language: Option<Language>,
    pub course: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeResult {
    pub status: String,
    pub duration: i32,
    pub mem_usage: i32,
    pub stdout: String,
    pub stderr: String,
    pub task_id: i32,
    pub case_id: i32,
    // exit_msg: String,
}

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

#[must_use]
pub fn status_str_to_i32(s: &str) -> i32 {
    match s {
        "AC" => 0,
        "WA" => 1,
        "CE" => 2,
        "TLE" => 3,
        "MLE" => 4,
        "RE" => 5,
        "JE" => 6,
        "OLE" => 7,
        _ => -2,
    }
}

impl ActiveModel {
    /// Update submission sandbox result
    ///
    /// # Errors
    ///
    /// When could not save the problem into DB
    #[allow(clippy::missing_panics_doc)]
    pub async fn update_sandbox_result<C: ConnectionTrait>(
        mut self,
        db: &C,
        problem: &problems::Model,
        results: Vec<Vec<JudgeResult>>,
    ) -> ModelResult<Model> {
        self.tasks = ActiveValue::set(Some(serde_json::to_value(&results).map_err(Box::from)?));

        let mut exec_time = i32::MAX;
        let mut memory_usage = i32::MAX;
        let mut score = 0;
        let mut status = -2;

        let tasks = problem.tasks(db).await?;

        for (task, rs) in tasks.iter().zip(&results) {
            #[allow(clippy::cast_sign_loss)]
            if rs.len() != task.test_case_count as usize {
                tracing::warn!("result dismatch");
            }

            if rs.iter().all(|r| r.status == "AC") {
                score += task.score;
            }

            for r in rs {
                // faster
                if (r.duration < exec_time)
                    // as fast, but less memory
                    || (r.duration == exec_time && r.mem_usage < memory_usage)
                {
                    exec_time = r.duration;
                    memory_usage = r.mem_usage;
                }

                let r_status = status_str_to_i32(&r.status);
                if r_status < 0 {
                    continue;
                }
                if status < 0 || r_status < status {
                    status = r_status;
                }
            }
        }

        self.status = ActiveValue::set(SubmissionStatus::try_from(status).unwrap());
        self.score = ActiveValue::set(score);
        self.exec_time = ActiveValue::set(exec_time);
        self.memory_usage = ActiveValue::set(memory_usage);
        Ok(self.update(db).await?)
    }

    /// Update submission code
    ///
    /// # Errors
    ///
    /// When could not save the problem into DB
    pub async fn update_code<C: ConnectionTrait>(
        mut self,
        db: &C,
        code: String,
    ) -> ModelResult<Model> {
        self.code = ActiveValue::set(code);
        Ok(self.update(db).await?)
    }
}

impl Model {
    /// Create a submission
    ///
    /// # Errors
    ///
    /// When could not save the problem into DB
    pub async fn add<C: ConnectionTrait + TransactionTrait>(
        db: &C,
        params: &AddParams,
    ) -> ModelResult<Self> {
        let txn = db.begin().await?;

        let submission = ActiveModel {
            user_id: ActiveValue::set(params.user),
            problem_id: ActiveValue::set(params.problem),
            timestamp: ActiveValue::set(params.timestamp),
            language: ActiveValue::set(params.language.clone()),
            ..Default::default()
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;

        Ok(submission)
    }

    /// List submissions
    ///
    /// # Errors
    ///
    /// When cloud not query submissions from DB
    pub async fn list<C: ConnectionTrait>(db: &C, params: &ListParams) -> ModelResult<Vec<Self>> {
        let mut q = Submissions::find().order_by(submissions::Column::Id, Order::Asc);

        if let Some(problem) = params.problem {
            q = q.filter(submissions::Column::ProblemId.eq(problem));
        }
        if let Some(user) = params.user {
            q = q.filter(submissions::Column::UserId.eq(user));
        }
        if let Some(status) = &params.status {
            q = q.filter(submissions::Column::Status.eq(status.clone()));
        }
        if let Some(language) = &params.language {
            q = q.filter(submissions::Column::Language.eq(language.clone()));
        }

        let submissions = q.all(db).await?.into_iter();

        let offset = params.offset.unwrap_or(0);
        let count = params.count.unwrap_or(usize::MAX);
        let submissions = submissions.skip(offset).take(count);

        Ok(submissions.collect())
    }

    /// Get submission by id
    ///
    /// # Errors
    ///
    /// When could not save the problem into DB
    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i32) -> ModelResult<Self> {
        Self::find_by_column(db, submissions::Column::Id, id).await
    }

    async fn find_by_column<C: ConnectionTrait>(
        db: &C,
        column: impl sea_orm::ColumnTrait,
        value: impl Into<sea_orm::Value> + Send,
    ) -> ModelResult<Self> {
        let submission = submissions::Entity::find()
            .filter(column.eq(value))
            .one(db)
            .await?;
        submission.ok_or(ModelError::EntityNotFound)
    }
}
