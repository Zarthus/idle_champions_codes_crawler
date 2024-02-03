use crate::handler::discord;
use licc::write::InsertCodeRequest;
use std::collections::HashMap;

mod cache;
mod client;
mod config;
mod handler;
mod parse;

#[macro_use]
extern crate log;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    zarthus_env_logger::init_named("liccrawler");

    let config = config::read();
    cache::setup();
    let mut cache = cache::read();

    let mut requests: HashMap<&str, Vec<InsertCodeRequest>> = HashMap::new();
    let mut responses: HashMap<String, Option<i32>> = HashMap::new();

    for (name, discord) in &config.discord {
        if discord.enabled {
            let outcome = discord::handle(discord).await;

            match outcome {
                Ok(out) => {
                    requests.insert("discord", out);

                    info!(
                        "Handled discord '{}' (Application ID: {})",
                        name, discord.application_id
                    );
                }
                Err(err) => {
                    error!("Error handling discord '{}': {:?}", name, err);
                }
            };
        } else {
            info!(
                "Skipping discord '{}', not enabled (Application ID: {})",
                name, discord.application_id
            );
        }
    }

    if config.dry_run {
        info!("Dry run enabled, not sending requests.");

        for (_, value) in requests {
            for request in value {
                responses.insert(request.code.clone(), None);
            }
        }
    } else {
        let mut client = config.client.client();

        for (from, value) in requests {
            for request in value {
                if cache.has(&request.code) {
                    info!("Skipping '{}' from {}, already stored.", request.code, from);
                    continue;
                }

                match client.insert_code(request.clone()).await {
                    Ok(response) => {
                        responses.insert(request.code.clone(), response);
                        cache.insert(request.code.clone());
                    }
                    Err(e) => {
                        responses.insert(request.code.clone(), None);
                        error!("Error ({}: {}): {:?}", from, request.code.clone(), e);
                    }
                }
            }
        }
    }

    for (code, response) in responses {
        match response {
            Some(num) => {
                info!("Stored '{}': {}", code, num);
            }
            None => {
                if config.dry_run {
                    info!("Stored '{}': No", code);
                } else {
                    warn!("Stored '{}': No", code);
                }
            }
        }
    }

    cache.bust();
    cache::write(cache);
}
