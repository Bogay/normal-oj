use std::path::Path;

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Hooks},
    boot::{create_app, BootResult, StartMode},
    controller::AppRoutes,
    db::{self, truncate_table},
    environment::Environment,
    storage::{self, Storage},
    task::Tasks,
    worker::{AppWorker, Processor},
    Result,
};
use migration::Migrator;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement};

use crate::{
    controllers,
    models::_entities::{
        courses, problem_descriptions, problem_tasks, problems, submissions, users,
    },
    tasks,
    workers::downloader::DownloadWorker,
};

pub struct App;
#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(mode: StartMode, environment: &Environment) -> Result<BootResult> {
        create_app::<Self, Migrator>(mode, environment).await
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
            .prefix("/api")
            .add_route(controllers::problems::routes())
            .add_route(controllers::courses::routes())
            .add_route(controllers::notes::routes())
            .add_route(controllers::auth::routes())
            .add_route(controllers::user::routes())
            .add_route(controllers::submissions::routes())
            .add_route(controllers::mock::routes())
    }

    fn connect_workers<'a>(p: &'a mut Processor, ctx: &'a AppContext) {
        p.register(crate::workers::submission::SubmissionWorker::build(ctx));
        p.register(DownloadWorker::build(ctx));
    }

    fn register_tasks(tasks: &mut Tasks) {
        tasks.register(tasks::seed::SeedData);
    }

    async fn truncate(db: &DatabaseConnection) -> Result<()> {
        truncate_table(db, submissions::Entity).await?;
        truncate_table(db, problem_tasks::Entity).await?;
        truncate_table(db, problems::Entity).await?;
        truncate_table(db, courses::Entity).await?;
        truncate_table(db, users::Entity).await?;
        truncate_table(db, problem_descriptions::Entity).await?;
        Ok(())
    }

    async fn seed(db: &DatabaseConnection, base: &Path) -> Result<()> {
        db::seed::<users::ActiveModel>(db, &base.join("users.yaml").display().to_string()).await?;
        db::seed::<courses::ActiveModel>(db, &base.join("courses.yaml").display().to_string())
            .await?;

        // update auto inc id
        // ref: https://stackoverflow.com/a/55024610
        // see also: https://github.com/loco-rs/loco/issues/239
        let tables = [
            "users",
            "courses",
            "problems",
            "problem_descriptions",
            "problem_tasks",
            "submissions",
        ];
        for table in tables {
            db.execute(Statement::from_string(
                DatabaseBackend::Postgres,
                format!(
                    "SELECT SETVAL('{table}_id_seq', (SELECT COALESCE(MAX(id), 1) FROM {table}))"
                ),
            ))
            .await?;
        }

        Ok(())
    }

    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        let store = if ctx.environment == Environment::Test {
            storage::drivers::mem::new()
        } else {
            storage::drivers::local::new_with_prefix("storage").map_err(Box::from)?
        };

        Ok(AppContext {
            storage: Storage::single(store).into(),
            ..ctx
        })
    }
}
