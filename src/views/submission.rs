use eyre::eyre;
use serde::Serialize;

use super::NojResponseBuilder;
use crate::models::{
    submissions::{self, Language, SubmissionStatus},
    users,
};
use crate::views::user::UserInfoResponse;

impl From<SubmissionStatus> for i32 {
    fn from(val: SubmissionStatus) -> Self {
        match val {
            SubmissionStatus::Pending => -1,
            SubmissionStatus::Accepted => 0,
            SubmissionStatus::WrongAnswer => 1,
            SubmissionStatus::ComileError => 2,
            SubmissionStatus::TimeLimitError => 3,
            SubmissionStatus::MemoryLimitError => 4,
            SubmissionStatus::RuntimeError => 5,
            SubmissionStatus::JudgeError => 6,
            SubmissionStatus::OutputLimitError => 7,
        }
    }
}

impl TryFrom<i32> for SubmissionStatus {
    type Error = eyre::Error;
    fn try_from(val: i32) -> Result<Self, eyre::Error> {
        match val {
            -1 => Ok(Self::Pending),
            0 => Ok(Self::Accepted),
            1 => Ok(Self::WrongAnswer),
            2 => Ok(Self::ComileError),
            3 => Ok(Self::TimeLimitError),
            4 => Ok(Self::MemoryLimitError),
            5 => Ok(Self::RuntimeError),
            6 => Ok(Self::JudgeError),
            7 => Ok(Self::OutputLimitError),
            _ => Err(eyre!("error submission type")),
        }
    }
}

impl From<Language> for i32 {
    fn from(val: Language) -> Self {
        match val {
            Language::C => 0,
            Language::Cpp => 1,
            Language::Python => 2,
        }
    }
}

impl TryFrom<i32> for Language {
    type Error = eyre::Error;
    fn try_from(val: i32) -> Result<Self, eyre::Error> {
        match val {
            0 => Ok(Self::C),
            1 => Ok(Self::Cpp),
            2 => Ok(Self::Python),
            _ => Err(eyre!("error language type")),
        }
    }
}

#[derive(Debug, Serialize)]
#[allow(clippy::module_name_repetitions)]
pub struct SubmissionListResponseItem {
    pub id: i32,
    pub user: UserInfoResponse,
    pub problem_id: i32,
    pub timestamp: i64,
    pub score: i32,
    pub exec_time: i32,
    pub memory_usage: i32,
    pub code: String,
    pub last_send: i64,
    pub status: i32,
    pub language: i32,
}

#[allow(clippy::module_name_repetitions)]
pub struct SubmissionListResponse {}

impl SubmissionListResponse {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        submissions: &[submissions::Model],
        users: &[users::Model],
    ) -> NojResponseBuilder<Vec<SubmissionListResponseItem>> {
        let data = submissions
            .iter()
            .zip(users)
            .map(|(p, u)| SubmissionListResponseItem {
                id: p.id,
                problem_id: p.problem_id,
                timestamp: p.timestamp.and_utc().timestamp(),
                score: p.score,
                exec_time: p.exec_time,
                status: p.status.clone().into(),
                language: p.language.clone().into(),
                last_send: p.last_send.and_utc().timestamp(),
                memory_usage: p.memory_usage,
                code: p.code.to_string(),
                user: UserInfoResponse::new(u),
            })
            .collect();

        NojResponseBuilder::new(data)
    }
}
