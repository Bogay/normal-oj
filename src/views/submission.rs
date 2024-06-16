use eyre::eyre;
use serde::{Deserialize, Serialize};

use super::NojResponseBuilder;
use crate::models::{
    problems,
    submissions::{self, status_str_to_i32, JudgeResult, Language, SubmissionStatus},
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
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct SubmissionListResponseItem {
    pub submission_id: i32,
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

#[derive(Debug, Serialize)]
#[allow(clippy::module_name_repetitions)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionListResponse {
    submissions: Vec<SubmissionListResponseItem>,
    submission_count: i32,
}

impl SubmissionListResponse {
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        submissions: &[submissions::Model],
        users: &[users::Model],
    ) -> NojResponseBuilder<Self> {
        let submissions = submissions
            .iter()
            .zip(users)
            .map(|(p, u)| SubmissionListResponseItem {
                submission_id: p.id,
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

        NojResponseBuilder::new(Self {
            submissions,
            // TODO: use real data
            submission_count: 123,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionCaseResponse {
    exec_time: i32,
    memory_usage: i32,
    status: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionTaskResponse {
    cases: Vec<SubmissionCaseResponse>,
    exec_time: i32,
    memory_usage: i32,
    score: i32,
    status: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmissionDetailResponse {
    code: String,
    ip_addr: String,
    language_type: i32,
    last_send: f64,
    problem_id: i32,
    memory_usage: i32,
    run_time: i32,
    score: i32,
    status: i32,
    submission_id: i32,
    tasks: Vec<SubmissionTaskResponse>,
    timestamp: f64,
    user: UserInfoResponse,
}

impl SubmissionDetailResponse {
    pub fn new(
        submission: &submissions::Model,
        user: &users::Model,
        problem_tasks: &[problems::tasks::Model],
    ) -> NojResponseBuilder<Self> {
        let tasks = submission.tasks.clone().unwrap_or(serde_json::json!([]));
        let tasks = serde_json::from_value::<Vec<Vec<JudgeResult>>>(tasks).unwrap();
        let tasks = problem_tasks
            .iter()
            .zip(&tasks)
            .map(|(pt, tt)| {
                let score = if tt.iter().all(|t| t.status == "AC") {
                    pt.score
                } else {
                    0
                };

                let cases = tt
                    .iter()
                    .map(|t| SubmissionCaseResponse {
                        memory_usage: t.mem_usage,
                        exec_time: t.duration,
                        status: status_str_to_i32(&t.status),
                    })
                    .collect::<Vec<_>>();

                let mut memory_usage = i32::MAX;
                let mut exec_time = i32::MAX;
                let mut status = -2;

                for r in tt {
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

                SubmissionTaskResponse {
                    cases,
                    exec_time,
                    memory_usage,
                    score,
                    status,
                }
            })
            .collect();

        let resp = Self {
            code: submission.code.to_string(),
            language_type: submission.language.clone().into(),
            last_send: submission.last_send.and_utc().timestamp() as f64,
            problem_id: submission.problem_id,
            status: submission.status.clone().into(),
            submission_id: submission.id,
            user: UserInfoResponse::new(user),
            timestamp: submission.timestamp.and_utc().timestamp() as f64,
            tasks,
            // TODO: use real data
            ip_addr: "127.0.0.1".to_string(),
            memory_usage: submission.memory_usage,
            run_time: submission.exec_time,
            score: submission.score,
        };

        NojResponseBuilder::new(resp)
    }
}
