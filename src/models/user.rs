use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub surname: String,
    #[sea_orm(unique)]
    pub email: String,
    pub password_hash: Option<String>,
    pub is_staff: bool,
    pub is_admin: bool,
    pub is_email_verified: bool,
    #[sea_orm(unique)]
    pub google_sub: Option<String>,
    #[sea_orm(unique)]
    pub referral_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::otp::Entity")]
    Otps,
    #[sea_orm(has_many = "super::refresh_token::Entity")]
    RefreshTokens,
    #[sea_orm(has_many = "super::password_reset_token::Entity")]
    PasswordResetTokens,
}

impl Related<super::otp::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Otps.def()
    }
}

impl Related<super::refresh_token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RefreshTokens.def()
    }
}

impl Related<super::password_reset_token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PasswordResetTokens.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
