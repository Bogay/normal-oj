mod auth;
mod prepare_data;
mod problems;
mod submissions;
mod user;

use loco_rs::app::AppContext;
use normal_oj::models::users;

pub async fn create_token(user: &users::Model, ctx: &AppContext) -> String {
    let jwt_secret = ctx.config.get_jwt_config().unwrap();
    user.generate_jwt(&jwt_secret.secret, &jwt_secret.expiration)
        .unwrap()
}
