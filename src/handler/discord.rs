use crate::config::DiscordConfig;
use crate::parse::{next_week, validate_code, TimeParser};
use licc::write::{InsertCodeRequest, SourceLookup};
use serenity::all::{ChannelId, GatewayIntents, MessageId, ReactionType};
use std::sync::Arc;

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
    let client: serenity::Client = client(cfg).await;

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
    let ack = cfg.acknowledge;
    let mut acks: Vec<MessageId> = vec![];
    let timeparser = TimeParser::new();

    for message in messages {
        if message.reactions.iter().any(|r| r.me) {
            trace!("Skipping message with existing reaction from self");
            continue;
        }

        let guild_id = message.guild_id.map(|g| g.get()).unwrap_or(cfg.guild_id);
        let channel_id = message.channel_id.get();
        let (code, expires_at, creator_name, creator_url) = match parse(
            message.content.clone(),
            message.timestamp.timestamp() as u64,
            &timeparser,
        ) {
            Ok(parsed) => parsed,
            Err(err) => {
                error!("Error parsing message {}: {}", message.id, err);
                error!("Message: {}", message.content);
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
        if ack {
            acks.push(message.id);
        }
    }

    for message_id in acks {
        acknowledge(client.http.clone(), channel_id, message_id).await;
    }

    Ok(codes)
}

async fn acknowledge(
    http: Arc<serenity::http::Http>,
    channel_id: ChannelId,
    message_id: MessageId,
) {
    // We don't need to handle the result here, we just want to log, as acknowledging is optional behaviour and not critical if fails,
    // in addition, it's an optional permission that the bot might not have. (though if it doesn't have it, you should probably turn it off in the config)
    http.create_reaction(channel_id, message_id, &ReactionType::from('👍'))
        .await
        .inspect_err(|e| error!("Error acknowledging message: {}", e))
        .inspect(|_| info!("Acknowledged message"))
        .ok();
}

async fn client(cfg: &DiscordConfig) -> serenity::Client {
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    serenity::Client::builder(&cfg.bot_token, intents)
        .await
        .expect("Error creating client")
}

fn parse(
    message: String,
    message_ts: u64,
    timeparser: &TimeParser,
) -> Result<(String, u64, String, String), &'static str> {
    let mut parts = message.split('\n');

    if parts.clone().count() < 3 {
        return Err("Likely unrecoverable message format");
    }

    let code = parts.next().unwrap().to_string().replace(' ', "");

    if !validate_code(&code) {
        return Err("Invalid code length");
    }

    let creator_name_fallback = parts.next();

    let creator_url = match parts.next() {
        Some(url) => url,
        None => return Err("Missing creator URL"),
    };

    // https://twitch.tv/foo -> foo
    let mut creator_name = creator_url
        .split('/')
        .last()
        .unwrap_or(creator_name_fallback.unwrap_or("Unknown"))
        .to_lowercase();
    // might be a youtube link
    if creator_name.contains('?') {
        debug!(
            "Creator name looks fishy, using fallback: {}",
            creator_name_fallback.unwrap_or("Unknown")
        );

        creator_name = creator_name_fallback.unwrap_or("Unknown").to_string();
    }

    parts.next();

    let expires_at = match parts.next() {
        None => next_week(),
        Some(txt) => timeparser
            .parse(txt.to_string(), true)
            .unwrap_or(message_ts + (60 * 24 * 7)),
    };

    Ok((code, expires_at, creator_name, creator_url.to_string()))
}

#[cfg(test)]
mod test {
    use super::*;

    macro_rules! test_inputs {
        () => {
            vec![
                "CODE-AAAA-BBBB\nTest Input\nhttps://www.twitch.tv/foo\n1x :bar:\nExpires Next Week",
                "CODE-AAAA-BBBB\nTest Input\nhttps://www.twitch.tv/foo\n1x :bar:\nExpires Jan 26th",
                "REPP-PERE-SEAN\nGaar slings some hash\nhttps://www.twitch.tv/gaarawarr\n1x :electrumchest:\nExpires Next Week",
                "EARD-EEZH-ERKS\nGina Darling - Idle Insights\nhttps://youtu.be/sNFoGtn-Qfw?si=j8PF5-tgMw6liltq\n1x :electrumchest:\nExpires Jan 26th"
            ]
        }
    }
    const DEFAULT_MESSAGE_TS: u64 = 1726221600;

    #[test]
    fn test_parse_many() {
        let tp = TimeParser::new();

        for input in test_inputs!() {
            let (code, expires_at, creator_name, creator_url) =
                parse(input.to_string(), DEFAULT_MESSAGE_TS, &tp).unwrap();
            assert!(!code.is_empty(), "Input: {}", input);
            assert!(expires_at > 0, "Input: {}", input);
            assert!(!creator_name.is_empty(), "Input: {}", input);
            assert!(!creator_url.is_empty(), "Input: {}", input);
        }
    }

    #[test]
    fn test_parse() {
        let tp = TimeParser::new();

        let input =
            "CODE-AAAA-BBBB\nTest Input\nhttps://www.twitch.tv/foo\n1x :bar:\nExpires WeDontKnow";
        let (code, expires_at, creator_name, creator_url) =
            parse(input.to_string(), 0, &tp).unwrap();

        assert_eq!(code, "CODE-AAAA-BBBB");
        assert_eq!(expires_at, 10080); // next week (60 * 24 * 7) added to the message timestamp (0 seconds)
        assert_eq!(creator_name, "foo");
        assert_eq!(creator_url, "https://www.twitch.tv/foo");
    }

    #[test]
    fn test_parse_youtube() {
        let tp = TimeParser::new();

        let input =
            "EARD-EEZH-ERKS-AAAA\nGina Darling - Idle Insights\nhttps://youtu.be/sNFoGtn-Qfw?si=j8PF5-tgMw6liltq\n1x :electrumchest:\nExpires Jan 26th";
        let (code, expires_at, creator_name, creator_url) =
            parse(input.to_string(), DEFAULT_MESSAGE_TS, &tp).unwrap();

        assert_eq!(code, "EARD-EEZH-ERKS-AAAA");
        assert_eq!(expires_at, 1706227200);
        assert_eq!(creator_name, "Gina Darling - Idle Insights");
        assert_eq!(
            creator_url,
            "https://youtu.be/sNFoGtn-Qfw?si=j8PF5-tgMw6liltq"
        );
    }

    #[test]
    fn test_parse_relative_time() {
        let tp = TimeParser::new();

        let input =
            "CODE-AAAA-BBBB\nTest Input\nhttps://www.twitch.tv/foo\n1x :bar:\nExpires Next Week";
        let (_code, expires_at, _creator_name, _creator_url) =
            parse(input.to_string(), DEFAULT_MESSAGE_TS, &tp).unwrap();

        assert_eq!(expires_at, next_week());
    }

    #[test]
    fn test_parse_absolute_time() {
        let tp = TimeParser::new();

        let input =
            "CODE-AAAA-BBBB\nTest Input\nhttps://www.twitch.tv/foo\n1x :bar:\nExpires Jan 26th";
        let (_code, expires_at, _creator_name, _creator_url) =
            parse(input.to_string(), DEFAULT_MESSAGE_TS, &tp).unwrap();

        assert_eq!(expires_at, 1706227200);
    }
}
