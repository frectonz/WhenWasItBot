use chrono::NaiveDateTime;
use serde::Serialize;
use std::convert::Infallible;
use std::env;
use warp::Filter;

#[derive(Serialize, Debug)]
struct SendMessageRequest {
    chat_id: i64,
    text: String,
    parse_mode: Option<String>,
    reply_to_message_id: Option<i64>,
}

#[tokio::main]
async fn main() {
    let token = env::var("TELEGRAM_BOT_TOKEN")
        .expect("TELEGRAM_BOT_TOKEN not found in environment variables");
    let api_url = format!("https://api.telegram.org/bot{token}");

    let webhook_url =
        env::var("WEBHOOK_URL").expect("WEBHOOK_URL not found in environment variables");
    let webhook_url = format!("{webhook_url}/telegram-webhook");

    // Register the webhook with Telegram
    let set_webhook_url = format!("{api_url}/setWebhook?url={webhook_url}");
    reqwest::get(&set_webhook_url).await.unwrap();

    let client = reqwest::Client::new();

    // Define the route for handling updates from Telegram
    let webhook_handler = warp::post()
        .and(warp::path("telegram-webhook"))
        .and(warp::any().map(move || client.clone()))
        .and(warp::any().map(move || api_url.clone()))
        .and(warp::body::json())
        .and_then(handle_webhook);

    let health_check = warp::get()
        .and(warp::path("health"))
        .map(|| warp::reply::json(&"OK"));

    // Start the server to listen for updates
    println!("Server started");
    warp::serve(webhook_handler.or(health_check))
        .run(([0, 0, 0, 0], 8080))
        .await;
}

async fn handle_webhook(
    client: reqwest::Client,
    api_url: String,
    json: serde_json::Value,
) -> Result<impl warp::Reply, Infallible> {
    let chat_id = json
        .get("message")
        .and_then(|m| m.get("chat").and_then(|c| c.get("id")));

    let message_id = json
        .get("message")
        .and_then(|m| m.get("message_id"))
        .or_else(|| json.get("edited_message").and_then(|m| m.get("message_id")));

    let chat_id = match chat_id {
        Some(chat_id) => chat_id.as_i64().unwrap(),
        None => return Ok(warp::reply()),
    };

    let maybe_timestamp = json
        .get("message")
        .or_else(|| json.get("edited_message"))
        .and_then(|m| m.get("forward_date"));

    match maybe_timestamp {
        Some(timestamp) => {
            let timestamp = match timestamp.as_i64() {
                Some(timestamp) => timestamp,
                None => return Ok(warp::reply()),
            };

            let date_string = match timestamp_to_date_string(timestamp) {
                Some(date_string) => date_string,
                None => return Ok(warp::reply()),
            };

            let request = SendMessageRequest {
                chat_id,
                text: format!("The message was sent on `{date_string}`"),
                parse_mode: Some("MarkdownV2".to_string()),
                reply_to_message_id: message_id.and_then(|m| m.as_i64()),
            };
            let send_message_url = format!("{api_url}/sendMessage");

            let _ = client.post(&send_message_url).json(&request).send().await;
        }
        None => {
            let request = SendMessageRequest {
                chat_id,
                text: "Could not find the date of the forwarded message".to_string(),
                parse_mode: None,
                reply_to_message_id: message_id.and_then(|m| m.as_i64()),
            };
            let send_message_url = format!("{api_url}/sendMessage");

            let _ = client.post(&send_message_url).json(&request).send().await;
        }
    };

    Ok(warp::reply())
}

fn timestamp_to_date_string(timestamp: i64) -> Option<String> {
    let naive_date_time = NaiveDateTime::from_timestamp_opt(timestamp, 0)?;
    Some(naive_date_time.format("%A, %B %d, %Y %H:%M:%S").to_string())
}
