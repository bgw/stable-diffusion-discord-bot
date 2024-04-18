use std::collections::HashSet;
use std::env;

use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use prompt_parse::prompt_parse;
use serde::Deserialize;
use serenity::async_trait;
use serenity::builder::CreateAttachment;
use serenity::builder::EditMessage;
use serenity::cache::Settings as CacheSettings;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::channel::Message;
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::ChannelId;
use tokio::fs;
use tracing::{debug, error, info, instrument};

use crate::stability_api::StabilityApi;

mod prompt_parse;
mod stability_api;

struct Handler {
    allowed_channel_ids: HashSet<ChannelId>,
    stability_api: StabilityApi,
}

impl Handler {
    async fn handle_message_impl(&self, ctx: Context, msg: Message) -> anyhow::Result<()> {
        debug!(%msg.author, %msg.content, "received message");

        let request = match prompt_parse(&msg.content) {
            Ok(req) => req,
            Err(err) => {
                msg.reply(&ctx, &err.to_string()).await?;
                return Err(err);
            }
        };

        let (reply_msg, response_results) =
            tokio::join!(msg.reply(&ctx, "*Processing...*"), async {
                let futures = FuturesUnordered::new();
                for _ in 0..3 {
                    futures.push(self.stability_api.text_to_image(request.clone()))
                }
                futures.collect::<Vec<_>>().await
            });

        let mut reply_msg = reply_msg?;

        let mut attachments = Vec::new();
        let mut errors = Vec::new();
        for result in response_results {
            match result {
                Ok(resp) => {
                    attachments.push(CreateAttachment::bytes(
                        resp.image,
                        "stablediffusionbot.png",
                    ));
                }
                Err(err) => {
                    errors.push(err);
                }
            }
        }

        let mut edit_msg = EditMessage::new().content("");
        if !errors.is_empty() {
            let errs_str = errors
                .iter()
                .map(|e| format!("> {e}"))
                .collect::<Vec<_>>()
                .join("\n");
            edit_msg = edit_msg.content(format!(
                "Encountered an error while processing your request. Please try again.\n{errs_str}"
            ))
        }
        for attach in attachments {
            edit_msg = edit_msg.new_attachment(attach);
        }
        reply_msg.edit(ctx.http, edit_msg).await?;

        if let Some(err) = errors.pop() {
            return Err(err);
        }
        Ok(())
    }
}

#[async_trait]
impl EventHandler for Handler {
    #[instrument(skip_all, fields(msg.id = %msg.id))]
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        if !self.allowed_channel_ids.contains(&msg.channel_id) {
            return;
        }

        if let Err(err) = self.handle_message_impl(ctx, msg).await {
            error!(%err);
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected", ready.user.name);
    }
}

#[derive(Deserialize)]
struct BotConfig {
    allowed_channels: Vec<ChannelId>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let config: BotConfig = toml::from_str(
        &fs::read_to_string("bot.toml")
            .await
            .expect("failed to read bot.toml"),
    )
    .expect("failed to parse bot.toml");

    let stability_api_key =
        env::var("STABILITY_API_KEY").expect("STABILITY_API_KEY environment variable must be set");
    let discord_token =
        env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN environment variable must be set");

    let allowed_channel_ids = HashSet::from_iter(config.allowed_channels.iter().copied());
    let handler = Handler {
        allowed_channel_ids,
        stability_api: StabilityApi::new(&stability_api_key),
    };

    let mut cache_settings = CacheSettings::default();
    cache_settings.max_messages = 100;

    let mut client = Client::builder(
        &discord_token,
        GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILD_MESSAGES,
    )
    .cache_settings(cache_settings)
    .event_handler(handler)
    .await
    .expect("error creating client");

    client.start().await.expect("error starting client");
}
