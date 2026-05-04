use crate::entities::{balances, prelude::*};
use crate::{AppState, handlers::auth::check_session};
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    pub slippage_bps: u32,
}

#[derive(Deserialize)]
pub struct SwapRequest {
    pub quote_response: serde_json::Value,
}

pub async fn get_quote(query: web::Query<QuoteRequest>) -> impl Responder {
    let api_key = std::env::var("JUPITER_API_KEY").unwrap_or_default();

    let url = format!(
        "https://api.jup.ag/swap/v1/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
        query.input_mint, query.output_mint, query.amount, query.slippage_bps
    );

    let client = reqwest::Client::new();
    let response = client.get(&url).header("x-api-key", &api_key).send().await;

    match response {
        Ok(res) => {
            // Check if Jupiter returned a 400 or 403 error first
            if !res.status().is_success() {
                let err_text = res.text().await.unwrap_or_default();
                println!("Jupiter API Error: {}", err_text);
                return HttpResponse::BadRequest()
                    .body(format!("Jupiter returned error: {}", err_text));
            }

            // Try to parse JSON safely
            match res.json::<serde_json::Value>().await {
                Ok(body) => HttpResponse::Ok().json(body),
                Err(e) => {
                    println!("JSON Parse Error: {:?}", e);
                    HttpResponse::InternalServerError().body("Failed to parse Jupiter response")
                }
            }
        }
        Err(e) => {
            // THIS IS WHERE YOUR CURRENT ERROR IS HITTING
            println!("Request Error: {:?}", e);
            HttpResponse::InternalServerError().body(format!("Request failed: {:?}", e))
        }
    }
}

pub async fn get_swap_tx(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<SwapRequest>,
) -> impl Responder {
    // 1. Session Verification
    let cookie = match req.cookie("session_token") {
        Some(c) => c.value().to_string(),
        None => return HttpResponse::Unauthorized().body("Missing session"),
    };

    let user = match check_session(&data.db, cookie).await {
        Some(u) => u,
        None => return HttpResponse::Unauthorized().body("Invalid or expired session"),
    };

    // 2. Fetch Wallet from DB (Hex format)
    let wallet = Balances::find()
        .filter(balances::Column::UserId.eq(user.user_id))
        .one(&data.db)
        .await
        .unwrap();

    let hex_pubkey = match wallet {
        Some(w) => w.pubkey,
        None => return HttpResponse::BadRequest().body("No wallet assigned to user"),
    };

    // 3. CONVERSION: Hex String -> Bytes -> Base58 String
    // Solana addresses must be Base58 for the Jupiter API
    let pubkey_bytes = match hex::decode(&hex_pubkey) {
        Ok(bytes) => bytes,
        Err(_) => {
            return HttpResponse::InternalServerError()
                .body("Failed to decode hex pubkey from database");
        }
    };

    let base58_pubkey = bs58::encode(pubkey_bytes).into_string();

    println!("Converted Hex to Base58: {}", base58_pubkey);

    // 4. Prepare Jupiter Request
    let api_key = env::var("JUPITER_API_KEY").unwrap_or_default();
    let client = reqwest::Client::new();

    let swap_payload = serde_json::json!({
        "quoteResponse": body.quote_response,
        "userPublicKey": base58_pubkey, // Use the converted Base58 address
        "wrapAndUnwrapSol": true,
        "dynamicComputeUnitLimit": true,
        "prioritizationFeeLamports": "auto"
    });

    let response = client
        .post("https://api.jup.ag/swap/v1/swap")
        .header("x-api-key", &api_key)
        .json(&swap_payload)
        .send()
        .await;

    // 5. Handle Jupiter Response
    match response {
        Ok(res) => {
            let status = res.status();
            let body_text = res.text().await.unwrap_or_default();

            if !status.is_success() {
                println!("Jupiter API Error ({}): {}", status, body_text);
                return HttpResponse::BadRequest().body(format!("Jupiter Error: {}", body_text));
            }

            match serde_json::from_str::<serde_json::Value>(&body_text) {
                Ok(jup_data) => HttpResponse::Ok().json(jup_data),
                Err(_) => HttpResponse::InternalServerError().body("Failed to parse Jupiter JSON"),
            }
        }
        Err(e) => {
            println!("Request Error: {:?}", e);
            HttpResponse::InternalServerError().body("Failed to reach Jupiter API")
        }
    }
}
