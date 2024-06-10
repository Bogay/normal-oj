use loco_rs::prelude::*;
use serde_json::json;

pub fn routes() -> Routes {
    Routes::new().add("/ann", get(|| async { format::json(json!({"data": []})) }))
}
