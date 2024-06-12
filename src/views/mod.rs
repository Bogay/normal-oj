pub mod auth;
pub mod problems;
pub mod submission;
pub mod user;

use loco_rs::controller::views::pagination::PagerMeta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub results: Vec<T>,
    pub pagination: PagerMeta,
}

#[derive(Debug, Serialize)]
pub struct NojResponse<T> {
    pub data: T,
    pub message: String,
}

#[derive(Debug)]
pub struct NojResponseBuilder<T> {
    pub data: T,
    pub message: String,
}

impl<T> NojResponseBuilder<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            message: String::new(),
        }
    }

    pub fn message(&mut self, message: String) -> &mut Self {
        self.message = message;
        self
    }

    pub fn done(self) -> NojResponse<T> {
        NojResponse {
            data: self.data,
            message: self.message,
        }
    }
}
