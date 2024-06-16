use sea_orm_migration::{
    prelude::*,
    schema::*,
    sea_orm::{EnumIter, Iterable},
    sea_query::extension::postgres::Type,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let language_alias = Alias::new("language");
        let status_alias = Alias::new("submission_status");

        manager
            .create_type(
                Type::create()
                    .as_enum(status_alias.clone())
                    .values(SubmissionStatus::iter())
                    .to_owned(),
            )
            .await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(language_alias.clone())
                    .values(SubmissionLanguage::iter())
                    .to_owned(),
            )
            .await?;

        let table = table_auto(Submissions::Table)
            .col(pk_auto(Submissions::Id))
            .foreign_key(
                ForeignKey::create()
                    .name("fk-submission-user_id")
                    .from(Submissions::Table, Submissions::UserId)
                    .to(Users::Table, Users::Id),
            )
            .col(integer(Submissions::UserId))
            .col(integer(Submissions::ProblemId))
            .col(timestamp(Submissions::Timestamp))
            .col(integer(Submissions::Score).default(0))
            .col(integer(Submissions::ExecTime).default(0))
            .col(integer(Submissions::MemoryUsage).default(0))
            .col(text(Submissions::Code).default(""))
            .col(
                timestamp(Submissions::LastSend)
                    .default(Expr::current_timestamp())
                    .not_null(),
            )
            .col(
                ColumnDef::new(Submissions::Status)
                    .enumeration(status_alias.clone(), SubmissionStatus::iter())
                    .default(SubmissionStatus::Pending.to_string())
                    .not_null(),
            )
            .col(
                ColumnDef::new(Submissions::Language)
                    .enumeration(language_alias.clone(), SubmissionLanguage::iter())
                    .not_null(),
            )
            .to_owned();

        manager.create_table(table).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Submissions::Table).to_owned())
            .await?;

        manager
            .drop_type(
                Type::drop()
                    .name(Alias::new("submission_status"))
                    .to_owned(),
            )
            .await?;

        manager
            .drop_type(Type::drop().name(Alias::new("language")).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Submissions {
    Table,
    Id,
    UserId,
    Score,
    Status,
    ProblemId,
    Language,
    Timestamp,
    ExecTime,
    MemoryUsage,
    Code,
    LastSend,
}

#[derive(DeriveIden, EnumIter)]
enum SubmissionStatus {
    Pending,
    Accepted,
    WrongAnswer,
    ComileError,
    TimeLimitError,
    MemoryLimitError,
    RuntimeError,
    JudgeError,
    OutputLimitError,
}

#[derive(DeriveIden, EnumIter)]
enum SubmissionLanguage {
    C,
    Cpp,
    Python,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
