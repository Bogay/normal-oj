pub mod descriptions;
pub mod tasks;
pub mod test_case;

use std::{collections::HashSet, path::PathBuf};

use super::_entities::{self, prelude::Problems, problems};
use crate::models::transform_db_error;

pub use _entities::problems::{ActiveModel, Model};
use axum::body::Bytes;
use loco_rs::model::{ModelError, ModelResult};
use num_derive::FromPrimitive;
use sea_orm::{entity::prelude::*, ActiveValue, Order, QueryOrder, TransactionTrait};
use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BadTestCase {
    #[error("error reading zip file: {0}")]
    ZipError(#[from] zip::result::ZipError),
    #[error("{0}")]
    Custom(String),
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("user is not permitted to this operation")]
    PermissionDenied,
    #[error("bad test cacse: {0}")]
    BadTestCase(BadTestCase),
    #[error("test case hasn't been uploaded for problem")]
    NoTestCase,
}

#[derive(Clone, Copy, Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq, FromPrimitive)]
#[repr(i8)]
pub enum Visibility {
    Show = 0,
    Hidden = 1,
}

#[derive(Clone, Copy, Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq, FromPrimitive)]
#[repr(i8)]
pub enum Type {
    Normal = 0,
    FillInTemplate = 1,
    Handwritten = 2,
}

impl ActiveModelBehavior for ActiveModel {
    // extend activemodel below (keep comment for generators)
}

#[derive(Debug, Deserialize)]
pub struct AddParams {
    pub owner: _entities::users::Model,
    /// list of course names
    pub courses: Vec<String>,
    pub name: String,
    /// Problem status, control its visibility
    pub status: Option<Visibility>,
    /// Problem description struct
    pub description: descriptions::AddParams,
    pub r#type: Option<Type>,
    pub allowed_language: Option<i32>,
    pub quota: Option<i32>,
    pub tasks: Vec<tasks::AddParams>,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub viewer: _entities::users::Model,
    pub offset: Option<usize>,
    /// how many problems to return, -1 to return all
    pub count: Option<i32>,
    pub name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub course: Option<String>,
}

impl _entities::problems::Model {
    /// Create a problem without test case binary
    ///
    /// # Errors
    ///
    /// - When could not save the problem into DB
    /// - When the owner is not a teacher or admin
    pub async fn add<C: ConnectionTrait + TransactionTrait>(
        db: &C,
        params: &AddParams,
    ) -> ModelResult<Self> {
        use super::users::Role;

        let txn = db.begin().await?;

        if !matches!(params.owner.role, Role::Teacher | Role::Admin) {
            return Err(ModelError::Any(Error::PermissionDenied.into()));
        }

        let description = descriptions::Model::add(&txn, &params.description).await?;

        let problem = ActiveModel {
            name: ActiveValue::set(params.name.to_string()),
            owner_id: ActiveValue::set(params.owner.id),
            r#type: params
                .r#type
                .map_or(ActiveValue::NotSet, |t| ActiveValue::set(t as i32)),
            status: params
                .status
                .map_or(ActiveValue::NotSet, |s| ActiveValue::set(s as i32)),
            description_id: ActiveValue::set(description.id),
            allowed_language: params
                .allowed_language
                .map_or(ActiveValue::NotSet, ActiveValue::set),
            quota: params.quota.map_or(ActiveValue::NotSet, ActiveValue::set),
            ..Default::default()
        }
        .insert(&txn)
        .await
        .map_err(transform_db_error)?;

        tasks::Model::add_many(&txn, problem.id, &params.tasks).await?;

        txn.commit().await.map_err(transform_db_error)?;

        Ok(problem)
    }

    /// List problems
    ///
    /// # Errors
    ///
    /// When cloud not query problems from DB
    pub async fn list<C: ConnectionTrait>(db: &C, params: &ListParams) -> ModelResult<Vec<Self>> {
        // TODO: check course && tags

        let mut q = Problems::find().order_by(problems::Column::Id, Order::Asc);

        // TODO: fuzz search
        if let Some(name) = &params.name {
            q = q.filter(problems::Column::Name.eq(name));
        }

        let problems = q.all(db).await?.into_iter();
        // TODO: permission check

        let offset = params.offset.unwrap_or(0);
        let count = params.count.unwrap_or(10);
        #[allow(clippy::cast_sign_loss)]
        let count = if count < 0 {
            usize::MAX
        } else {
            count as usize
        };
        let problems = problems.skip(offset).take(count);

        Ok(problems.collect())
    }

    /// Find a problem by its primary id
    ///
    /// # Errors
    ///
    /// - When cloud not query problem from DB
    /// - When the problem with id does not exist
    pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i32) -> ModelResult<Self> {
        let p = Problems::find()
            .filter(problems::Column::Id.eq(id))
            .one(db)
            .await
            .map_err(transform_db_error)?;
        p.ok_or(ModelError::EntityNotFound)
    }

    /// Find problem tasks from DB
    ///
    /// # Errors
    ///
    /// When there is DB error.
    pub async fn tasks<C: ConnectionTrait>(&self, db: &C) -> ModelResult<Vec<tasks::Model>> {
        let tasks = self
            .find_related(super::_entities::problem_tasks::Entity)
            .order_by(super::_entities::problem_tasks::Column::Id, Order::Asc)
            .all(db)
            .await?;

        Ok(tasks)
    }

    /// Validate test case binary according to current problem test case meta.
    ///
    /// # Errors
    ///
    /// - When the given binary is not a zip file
    /// - When the zip file contains invalid files
    /// - When there is missing/extra files inside zip
    pub async fn validate_test_case<C: ConnectionTrait>(
        &self,
        db: &C,
        test_case: &Bytes,
    ) -> loco_rs::Result<()> {
        let wrap_zip_error =
            |e| loco_rs::Error::Any(Box::new(Error::BadTestCase(BadTestCase::ZipError(e))));
        let custom_error =
            |e| loco_rs::Error::Any(Box::new(Error::BadTestCase(BadTestCase::Custom(e))));

        let cursor = std::io::Cursor::new(test_case);
        let mut zipfile = zip::ZipArchive::new(cursor).map_err(wrap_zip_error)?;

        // TODO: valiadte according to problem test case meta
        let tasks = self.tasks(db).await?;
        let mut expected_input_output = tasks
            .iter()
            .enumerate()
            .flat_map(|(i, t)| {
                (0..t.test_case_count).flat_map(move |j| {
                    vec![
                        format!("test-case/{i:02}{j:02}/STDIN"),
                        format!("test-case/{i:02}{j:02}/STDOUT"),
                    ]
                })
            })
            .collect::<HashSet<_>>();

        for i in 0..zipfile.len() {
            let file = zipfile.by_index(i).map_err(wrap_zip_error)?;
            if file.is_symlink() {
                return Err(custom_error(format!(
                    "symlink is not allowed: {}",
                    file.name()
                )));
            }
            // skip directory for now
            if file.is_dir() {
                continue;
            }
            let name = file.enclosed_name().ok_or_else(|| {
                custom_error(format!("invalid path found in zip file: {}", file.name()))
            })?;
            let name = name.to_str().ok_or_else(|| {
                custom_error(format!(
                    "invalid path found in zip file (maybe non-UTF8 path?): {}",
                    file.name()
                ))
            })?;

            if !expected_input_output.remove(name) {
                return Err(custom_error(format!(
                    "duplicated or extra file found: {}",
                    file.name()
                )));
            }
        }

        if !expected_input_output.is_empty() {
            return Err(custom_error(format!(
                "missing files: {}",
                expected_input_output
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(",")
            )));
        }

        Ok(())
    }

    #[must_use]
    pub fn test_case_path(&self) -> Option<PathBuf> {
        self.test_case_id
            .as_ref()
            .map(|i| PathBuf::from("test-case").join(format!("{i}.zip")))
    }
}

impl ActiveModel {
    /// Update test case id. The actual file content is handled by app's storage.
    ///
    /// # Errors
    ///
    /// When has DB query error.
    pub async fn update_test_case_id(
        mut self,
        db: &impl ConnectionTrait,
        test_case_id: Option<String>,
    ) -> ModelResult<Model> {
        self.test_case_id = ActiveValue::set(test_case_id);
        Ok(self.update(db).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_serde_visibility() {
        assert_eq!(json!(Visibility::Show), json!(0));
        assert_eq!(json!(Visibility::Hidden), json!(1));
        assert_eq!(
            Visibility::Show,
            serde_json::from_str::<Visibility>("0").unwrap(),
        );
        assert_eq!(
            Visibility::Hidden,
            serde_json::from_str::<Visibility>("1").unwrap(),
        );
        assert!(serde_json::from_str::<Visibility>("3").is_err());
    }
}
