mod migrations;

pub use sea_orm_migration::prelude::*;

use migrations::m20250107_122803_create_subscriptions_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(
            m20250107_122803_create_subscriptions_table::Migration,
        )]
    }
}