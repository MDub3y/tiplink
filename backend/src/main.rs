use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use sea_orm::{Database, DatabaseConnection};
use dotenvy::dotenv;
use std::env;

mod entities;

pub struct AppState {
    pub db: DatabaseConnection,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");

    let db: DatabaseConnection = Database::connect(db_url).await.expect("Failed to connect to DB!");

    println!("Tiplink Gateway connected to postgres");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState { db: db.clone() }))
            .route("/health", web::get().to(health_check))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("Service is healthy!")
}