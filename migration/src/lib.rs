mod m20250107_122803_create_subscriptions_table;
mod m20250112_124700_create_subscription_tokens_table;

pub use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250107_122803_create_subscriptions_table::Migration),
            Box::new(m20250112_124700_create_subscription_tokens_table::Migration),
        ]
    }
}
