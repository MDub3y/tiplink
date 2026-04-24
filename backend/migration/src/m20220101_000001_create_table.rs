use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        
        manager.create_table(
            Table::create().table(Users::Table).if_not_exists()
                .col(ColumnDef::new(Users::UserId).uuid().not_null().primary_key())
                .col(ColumnDef::new(Users::Email).string().unique_key().not_null())
                .col(ColumnDef::new(Users::PasswordHash).string().not_null())
                .col(ColumnDef::new(Users::Cookie).string())
                .to_owned(),
        ).await?;

        manager.create_table(
            Table::create().table(Balances::Table).if_not_exists()
                .col(ColumnDef::new(Balances::Pubkey).string().not_null().primary_key())
                .col(ColumnDef::new(Balances::Tokens).string().not_null())
                .col(ColumnDef::new(Balances::Balance).double().not_null().default(0.0))
                .col(ColumnDef::new(Balances::UserId).uuid())
                .to_owned(),
        ).await?;

        manager.create_table(
            Table::create().table(AssetPassword::Table).if_not_exists()
                .col(ColumnDef::new(AssetPassword::Pubkey).string().not_null().primary_key())
                .col(ColumnDef::new(AssetPassword::Hash).string().not_null())
                .to_owned(),
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Balances::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(AssetPassword::Table).to_owned()).await
    }
}

#[derive(Iden)] enum Users { Table, UserId, Email, PasswordHash, Cookie }
#[derive(Iden)] enum Balances { Table, Pubkey, Tokens, Balance, UserId }
#[derive(Iden)] enum AssetPassword { Table, Pubkey, Hash }
