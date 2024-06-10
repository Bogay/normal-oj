use serde::{Deserialize, Serialize};

use crate::models::users;

#[derive(Debug, Deserialize, Serialize)]
pub struct CurrentResponse {
    pub pid: String,
    pub name: String,
    pub email: String,
}

impl CurrentResponse {
    #[must_use]
    pub fn new(user: &users::Model) -> Self {
        Self {
            pid: user.pid.to_string(),
            name: user.name.clone(),
            email: user.email.clone(),
        }
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoResponse {
    pub username: String,
    pub displayed_name: String,
    // TODO: maybe we should use str to represent role?
    pub role: i32,
}

impl UserInfoResponse {
    #[must_use]
    pub fn new(user: &users::Model) -> Self {
        Self {
            username: user.name.clone(),
            // TODO: migration
            displayed_name: user.name.clone(),
            role: user.role.clone() as i32,
        }
    }
}
