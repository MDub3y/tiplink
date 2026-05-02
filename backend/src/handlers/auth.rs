use crate::AppState;
use crate::entities::{prelude::*, *};
use actix_web::{HttpResponse, Responder, cookie::Cookie, cookie::time::Duration, web};
use argon2::password_hash::SaltString;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, rand_core::OsRng},
};
use chrono::{Duration as ChronoDuration, Utc};
use sea_orm::*;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    pub username: String,
    pub asset_password: String,
}

#[derive(Deserialize)]
pub struct SigninRequest {
    pub email: String,
    pub password: String,
}

pub async fn signup(data: web::Data<AppState>, req: web::Json<SignupRequest>) -> impl Responder {
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

    match available_wallet {
        Some(wallet) => {
            let pubkey = wallet.pubkey.clone();

            let mut wallet_active: balances::ActiveModel = wallet.into();
            wallet_active.user_id = Set(Some(user_id));
            wallet_active.update(&txn).await.unwrap();

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

pub async fn signin(data: web::Data<AppState>, req: web::Json<SigninRequest>) -> impl Responder {
    let db = &data.db;

    let user = match Users::find()
        .filter(users::Column::Email.eq(req.email.clone()))
        .one(db)
        .await
        .unwrap()
    {
        Some(u) => u,
        None => return HttpResponse::Unauthorized().body("Invalild email or password"),
    };

    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().body("Error parsing stored hash"),
    };

    if let Err(_) = Argon2::default().verify_password(req.password.as_bytes(), &parsed_hash) {
        return HttpResponse::Unauthorized().body("Invalid Email or Password");
    }

    let expiry = Utc::now() + ChronoDuration::hours(24);
    let timestamp = expiry.timestamp();
    let uuid = Uuid::new_v4().to_string();

    let composite_token = format!("{}:{}", timestamp, uuid);

    let username = user.username.clone();

    let mut user_active: users::ActiveModel = user.into();
    user_active.cookie = Set(Some(composite_token.clone()));

    if let Err(_) = user_active.update(db).await {
        return HttpResponse::InternalServerError().body("Failed to create session");
    }

    let auth_cookie = Cookie::build("session_token", composite_token)
        .path("/")
        .http_only(true)
        .max_age(Duration::days(1))
        .finish();

    HttpResponse::Ok()
        .cookie(auth_cookie)
        .json(serde_json::json!({
            "status": "success",
            "username": username
        }))
}

pub async fn check_session(db: &DatabaseConnection, token: String) -> Option<users::Model> {
    let user = Users::find()
        .filter(users::Column::Cookie.eq(token.clone()))
        .one(db)
        .await
        .ok()??;

    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let expiry_timestamp: i64 = parts[0].parse().ok()?;

    if Utc::now().timestamp() > expiry_timestamp {
        return None;
    }

    Some(user)
}
