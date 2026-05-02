use sea_orm::entity::prelude::*;

pub mod key_share {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "key_shares")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub pubkey: String,
        pub share_blob: Vec<u8>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}