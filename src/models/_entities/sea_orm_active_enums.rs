//! `SeaORM` Entity. Generated by sea-orm-codegen 0.12.15

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "language")]
pub enum Language {
    #[sea_orm(string_value = "c")]
    C,
    #[sea_orm(string_value = "cpp")]
    Cpp,
    #[sea_orm(string_value = "python")]
    Python,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "role")]
pub enum Role {
    #[sea_orm(string_value = "admin")]
    Admin,
    #[sea_orm(string_value = "student")]
    Student,
    #[sea_orm(string_value = "teacher")]
    Teacher,
}
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "submission_status")]
pub enum SubmissionStatus {
    #[sea_orm(string_value = "accepted")]
    Accepted,
    #[sea_orm(string_value = "comile_error")]
    ComileError,
    #[sea_orm(string_value = "judge_error")]
    JudgeError,
    #[sea_orm(string_value = "memory_limit_error")]
    MemoryLimitError,
    #[sea_orm(string_value = "output_limit_error")]
    OutputLimitError,
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "runtime_error")]
    RuntimeError,
    #[sea_orm(string_value = "time_limit_error")]
    TimeLimitError,
    #[sea_orm(string_value = "wrong_answer")]
    WrongAnswer,
}
