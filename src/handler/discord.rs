use crate::config::DiscordConfig;
use crate::parse::{next_week, parse_user_expires_string, validate_code};
use licc::write::{InsertCodeRequest, SourceLookup};
use serenity::all::{ChannelId, GatewayIntents};

#[derive(Debug)]
pub enum DiscordError {
    MissingConfig,
    Serenity(serenity::Error),
}

pub async fn handle(cfg: &DiscordConfig) -> Result<Vec<InsertCodeRequest>, DiscordError> {
    if !cfg.enabled || cfg.bot_token.is_empty() || cfg.channel_id == 0 {
        return Err(DiscordError::MissingConfig);
    }

    let channel_id = ChannelId::new(cfg.channel_id);
    let client = client(cfg).await;

    let auth = client
        .http
        .get_current_user()
        .await
        .map_err(DiscordError::Serenity)?;

    debug!("Logged in as: {}", auth.name);

    let messages = client
        .http
        .get_messages(channel_id, None, Some(25))
        .await
        .map_err(DiscordError::Serenity)?;

    let mut codes: Vec<InsertCodeRequest> = vec![];

    for message in messages {
        let guild_id = message.guild_id.map(|g| g.get()).unwrap_or(cfg.guild_id);
        let channel_id = message.channel_id.get();
        let (code, expires_at, creator_name, creator_url) =
            match parse(message.content, message.timestamp.timestamp() as u64) {
                Ok(parsed) => parsed,
                Err(err) => {
                    error!("Error parsing message: {}", err);
                    continue;
                }
            };

        codes.push(InsertCodeRequest {
            code,
            expires_at,
            creator: SourceLookup {
                name: creator_name,
                url: creator_url,
            },
            submitter: Some(SourceLookup {
                name: message.author.global_name.unwrap_or(message.author.name),
                url: format!("https://discord.com/channels/{guild_id}/{channel_id}"),
            }),
        });
    }

    Ok(codes)
}

async fn client(cfg: &DiscordConfig) -> serenity::Client {
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    serenity::Client::builder(&cfg.bot_token, intents)
        .await
        .expect("Error creating client")
}

fn parse(message: String, message_ts: u64) -> Result<(String, u64, String, String), &'static str> {
    let mut parts = message.split('\n');

    let code = parts.next().unwrap().to_string().replace(' ', "");

    if !validate_code(&code) {
        return Err("Invalid code length");
    }

    let creator_name_default = parts.next();

    let creator_url = match parts.next() {
        Some(url) => url,
        None => return Err("Missing creator URL"),
    };

    // https://twitch.tv/foo -> foo
    let mut creator_name = creator_url.split('/').last().unwrap().to_lowercase();
    // might be a youtube link
    if creator_name.contains('?') {
        debug!(
            "Creator name looks fishy, using default: {}",
            creator_name_default.unwrap_or("Unknown")
        );

        creator_name = creator_name_default.unwrap_or("Unknown").to_string();
    }

    parts.next();

    let expires_at = match parts.next() {
        None => next_week(),
        Some(txt) => {
            parse_user_expires_string(txt.to_string()).unwrap_or(message_ts + 60 * 60 * 24 * 7)
        }
    };

    Ok((code, expires_at, creator_name, creator_url.to_string()))
}
