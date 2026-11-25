use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Otps::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Otps::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Otps::UserId).uuid().not_null())
                    .col(ColumnDef::new(Otps::CodeHash).string().not_null())
                    .col(ColumnDef::new(Otps::Purpose).string().not_null())
                    .col(
                        ColumnDef::new(Otps::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Otps::ConsumedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Otps::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Otps::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(5),
                    )
                    .col(
                        ColumnDef::new(Otps::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_otps_user_id")
                            .from(Otps::Table, Otps::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_otps_user_purpose")
                    .table(Otps::Table)
                    .col(Otps::UserId)
                    .col(Otps::Purpose)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Otps::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Otps {
    Table,
    Id,
    UserId,
    CodeHash,
    Purpose,
    ExpiresAt,
    ConsumedAt,
    Attempts,
    MaxAttempts,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
