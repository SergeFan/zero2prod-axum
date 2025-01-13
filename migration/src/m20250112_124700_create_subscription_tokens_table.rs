use sea_orm_migration::{prelude::*, schema::*};

use crate::m20250107_122803_create_subscriptions_table::Subscriptions;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .create_table(
                Table::create()
                    .table(SubscriptionTokens::Table)
                    .if_not_exists()
                    .col(text(SubscriptionTokens::SubscriptionToken).primary_key())
                    .col(uuid(SubscriptionTokens::SubscriberId))
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .from(SubscriptionTokens::Table, SubscriptionTokens::SubscriberId)
                    .to(Subscriptions::Table, Subscriptions::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        manager
            .drop_table(Table::drop().table(SubscriptionTokens::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SubscriptionTokens {
    Table,
    SubscriptionToken,
    SubscriberId,
}
