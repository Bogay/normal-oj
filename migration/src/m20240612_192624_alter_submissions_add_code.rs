use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Submissions {
    Table,
    Code,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Submissions::Table)
                    .add_column_if_not_exists(string_len(Submissions::Code, 16 * (1 << 20)).null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Submissions::Table)
                    .drop_column(Submissions::Code)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
