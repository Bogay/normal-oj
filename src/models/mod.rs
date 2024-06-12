pub mod _entities;
pub mod courses;
pub mod language;
pub mod notes;
pub mod problems;
pub mod submissions;
pub mod users;

pub use language::Language;

use loco_rs::model::ModelError;
use sea_orm::{DbErr, SqlErr};

pub(crate) fn is_unique_constraint_violation_err(e: &DbErr) -> bool {
    matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_)))
}

pub(crate) fn transform_db_error(e: DbErr) -> ModelError {
    if is_unique_constraint_violation_err(&e) {
        ModelError::EntityAlreadyExists {}
    } else {
        ModelError::Any(e.into())
    }
}
