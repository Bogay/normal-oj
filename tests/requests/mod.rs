mod auth;
mod prepare_data;
mod problems;
mod submissions;
mod user;

use loco_rs::{
    app::AppContext,
    prelude::cookie::{Cookie, SameSite},
};
use normal_oj::models::users;

pub async fn create_token(user: &users::Model, ctx: &AppContext) -> String {
    let jwt_secret = ctx.config.get_jwt_config().unwrap();
    user.generate_jwt(&jwt_secret.secret, &jwt_secret.expiration)
        .unwrap()
}

pub fn create_cookie(token: &str) -> Cookie {
    let mut c = Cookie::new("piann", token);
    c.set_http_only(true);
    c.set_path("/");
    c.set_same_site(SameSite::Lax);
    c.set_expires(time::OffsetDateTime::now_utc() + time::Duration::seconds(10086));
    c
}
