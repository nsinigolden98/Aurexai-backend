pub use sea_orm_migration::prelude::*;

mod m20260525_000001_create_users_table;
mod m20260525_000002_create_otps_table;
mod m20260525_000003_create_refresh_tokens_table;
mod m20260525_000004_create_password_reset_tokens_table;
mod m20260525_000005_add_referral_code_to_users;
mod m20260525_000006_create_notifications_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260525_000001_create_users_table::Migration),
            Box::new(m20260525_000002_create_otps_table::Migration),
            Box::new(m20260525_000003_create_refresh_tokens_table::Migration),
            Box::new(m20260525_000004_create_password_reset_tokens_table::Migration),
            Box::new(m20260525_000005_add_referral_code_to_users::Migration),
            Box::new(m20260525_000006_create_notifications_table::Migration),
        ]
    }
}
