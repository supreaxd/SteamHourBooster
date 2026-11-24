use std::time::Duration;
use steam_vent::{Connection, ConnectionTrait};
use steam_vent_proto_steam::steammessages_clientserver_2::CMsgClientRequestFreeLicense;
use crate::logger;

pub async fn claim_free_licenses(connection: &Connection, username: &str, appids: Vec<u32>) -> Vec<u32> {
    if appids.is_empty() {
        return Vec::new();
    }

    let mut msg = CMsgClientRequestFreeLicense::new();
    msg.appids = appids.clone();

    match connection.send(msg).await {
        Ok(()) => {
            logger::log_success(&format!(
                "🎁 [{}] Free game/package license requested ({} AppIDs: {:?})",
                username, appids.len(), appids
            ));
            appids
        }
        Err(e) => {
            logger::log_warn(&format!(
                "[{}] Error requesting free license: {}",
                username, e
            ));
            Vec::new()
        }
    }
}

pub async fn find_featured_free_games() -> Vec<(u32, String)> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(_) => reqwest::Client::new(),
    };

    let url = "https://store.steampowered.com/api/featuredcategories?cc=TR&l=turkish";
    let mut free_games = Vec::new();

    if let Ok(resp) = client.get(url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            // Check specials / genres / spotlights
            if let Some(specials) = json.get("specials").and_then(|s| s.get("items")).and_then(|i| i.as_array()) {
                for item in specials {
                    let discount_percent = item.get("discount_percent").and_then(|d| d.as_u64()).unwrap_or(0);
                    let final_price = item.get("final_price").and_then(|p| p.as_u64()).unwrap_or(1);
                    if discount_percent == 100 || final_price == 0 {
                        if let (Some(id), Some(name)) = (
                            item.get("id").and_then(|i| i.as_u64()),
                            item.get("name").and_then(|n| n.as_str()),
                        ) {
                            free_games.push((id as u32, name.to_string()));
                        }
                    }
                }
            }
        }
    }

    free_games
}
