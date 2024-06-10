use loco_rs::prelude::*;
use serde_json::json;

async fn empty_array() -> Result<Response> {
    format::json(json!({"data": []}))
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/ann", get(empty_array))
        .add("/course/:course_name/ann", get(empty_array))
        .add("/course/:course_name/homework", get(empty_array))
        .add(
            "/course",
            get(|| async {
                format::json(
                    json!({"data": [{"course": "Public", "teacher": {"username": "first_admin" }}]}),
                )
            }),
        )
}
