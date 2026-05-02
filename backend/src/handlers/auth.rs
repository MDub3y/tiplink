use actix_web::{web, HttpResponse, Responder};
use argon2::password_hash::SaltString;
use crate::entities::{prelude::*, *};
use crate::AppState;
use sea_orm::*;
use uuid::Uuid;
use argon2::{
    password_hash::{ rand_core::OsRng, PasswordHasher},
    Argon2,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    pub username: String,
    pub asset_password: String
}

pub async fn signup (
    data: web::Data<AppState>,
    req: web::Json<SignupRequest>,
) -> impl Responder {
    let db = &data.db;

    let salt = SaltString::generate(&mut OsRng);
    let hasher = Argon2::default();

    let password_hash = match hasher.hash_password(req.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return HttpResponse::InternalServerError().body("Error hashing password"),
    };

    let asset_hash = match hasher.hash_password(req.asset_password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return HttpResponse::InternalServerError().body("Error hashing asset password"),
    };

    let txn = match db.begin().await {
        Ok(t) => t,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let user_id = Uuid::new_v4();
    let new_user = users::ActiveModel {
        user_id: Set(user_id),
        email: Set(req.email.clone()),
        password_hash: Set(password_hash),
        username: Set(req.username.clone()),
        ..Default::default()
    };

    if let Err(e) = Users::insert(new_user).exec(&txn).await {
        return HttpResponse::BadRequest().body(format!("Username or Email taken: {}", e));
    }

    let available_wallet = Balances::find()
        .filter(balances::Column::UserId.is_null())
        .one(&txn)
        .await
        .unwrap();

        let available_wallet = Balances::find()
        .filter(balances::Column::UserId.is_null())
        .one(&txn)
        .await
        .unwrap();

    match available_wallet {
        Some(wallet) => {
            let pubkey = wallet.pubkey.clone();
            
            // Link wallet to user
            let mut wallet_active: balances::ActiveModel = wallet.into();
            wallet_active.user_id = Set(Some(user_id));
            wallet_active.update(&txn).await.unwrap();

            // 5. Store the Asset Password Hash linked to this specific pubkey
            let asset_pwd = asset_password::ActiveModel {
                pubkey: Set(pubkey.clone()),
                hash: Set(asset_hash),
            };
            AssetPassword::insert(asset_pwd).exec(&txn).await.unwrap();

            txn.commit().await.unwrap();

            HttpResponse::Ok().json(serde_json::json!({
                "message": "Signup successful",
                "pubkey": pubkey,
                "username": req.username
            }))
        }
        None => {
            txn.rollback().await.unwrap();
            HttpResponse::InternalServerError().body("No pre-generated wallets available!")
        }
    }
}