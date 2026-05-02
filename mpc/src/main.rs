mod entities;
use sea_orm::*;
use frost_ed25519 as frost;
use rand::thread_rng;
use dotenvy::dotenv;
use std::env;
use crate::entities::key_share;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let mut rng = thread_rng();

    // 1. Establish Connections
    let mpc_url1 = env::var("MPC_DB_1_URL").expect("MPC_DB_1_URL not set");
    let mpc_url2 = env::var("MPC_DB_2_URL").expect("MPC_DB_2_URL not set");
    let mpc_url3 = env::var("MPC_DB_3_URL").expect("MPC_DB_3_URL not set");
    let backend_url = env::var("BACKEND_DB_URL").expect("BACKEND_DB_URL not set");

    let mpc_db1 = Database::connect(mpc_url1).await?;
    let mpc_db2 = Database::connect(mpc_url2).await?;
    let mpc_db3 = Database::connect(mpc_url3).await?;
    let backend_db = Database::connect(backend_url).await?;

    let mpc_dbs = vec![&mpc_db1, &mpc_db2, &mpc_db3];

    println!("🗝️  Ceremony Started: Generating 100 sharded wallets (v0.3.0)...");

    for i in 1..=100 {
        // 2. Generate the 2-of-3 split
        let (shares, pubkey_package) = frost::keys::generate_with_dealer(3, 2, &mut rng)?;
        
        // 3. VerifyingKey (the pubkey) has a public to_bytes() method
        let pubkey_bytes = pubkey_package.group_public.to_bytes();
        let pubkey_hex = hex::encode(pubkey_bytes);

        // 4. Sort and Store Shards
        let mut share_list: Vec<_> = shares.into_iter().collect();
        // Sort by identifier to ensure Node 1 always gets Share 1, etc.
        share_list.sort_by_key(|(id, _)| *id);

        for (idx, (_id, share)) in share_list.into_iter().enumerate() {
            // The compiler confirmed 'share.value' has 'to_bytes()'.
            // This is the actual secret scalar we need to store.
            let blob = share.value.to_bytes().to_vec();
            
            let active_share = key_share::ActiveModel {
                pubkey: Set(pubkey_hex.clone()),
                share_blob: Set(blob),
            };
            
            key_share::Entity::insert(active_share).exec(mpc_dbs[idx]).await?;
        }

        // 5. Backend Registration
        let sql = format!(
            "INSERT INTO balances (pubkey, tokens, balance) VALUES ('{}', 'SOL', 0.0)",
            pubkey_hex
        );
        
        backend_db.execute_unprepared(&sql).await?;

        if i % 10 == 0 { println!("🚀 Progress: {}/100", i); }
    }

    println!("✅ Ceremony Complete. All systems synced.");
    Ok(())
}