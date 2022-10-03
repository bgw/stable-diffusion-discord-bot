mod prompt_parse;
mod sd_api;

use std::collections::HashSet;
use std::env;

use anyhow::Context as AnyhowContext;
use prompt_parse::prompt_parse;
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::channel::Message;
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::ChannelId;
use tokio::fs;
use tracing::{debug, error, info, instrument};

use crate::sd_api::{StableDiffusionApi, STABLE_DIFFUSION_VERSION};

struct Handler {
    allowed_channel_ids: HashSet<ChannelId>,
    sd_api: StableDiffusionApi,
}

impl Handler {
    async fn predict_message_impl(&self, ctx: Context, msg: Message) -> anyhow::Result<()> {
        debug!(%msg.author, %msg.content, "received message");

        let sd_request = match prompt_parse(&msg.content) {
            Ok(req) => req,
            Err(err) => {
                msg.reply(&ctx, &err).await?;
                return Err(err);
            }
        };

        let (reply_msg, img_uri_result) = tokio::join!(msg.reply(&ctx, "*Processing...*"), async {
            let mut uri_vec: Vec<String> = self
                .sd_api
                .predict(STABLE_DIFFUSION_VERSION, &sd_request)
                .await?;
            uri_vec
                .pop()
                .context("expected at least one result from replicate")
        });

        let mut reply_msg = reply_msg?;

        let img_uri = match img_uri_result {
            Ok(uri) => uri,
            Err(err) => {
                reply_msg
                    .edit(ctx, |edit_msg| {
                        edit_msg.content(format!(
                            "Encountered an error while processing your request. \
                            Please try again.\n\
                            > {err}"
                        ))
                    })
                    .await?;
                return Err(err);
            }
        };

        reply_msg
            .edit(ctx, |edit_msg| {
                edit_msg.content("").attachment(img_uri.as_str())
            })
            .await?;
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

        if let Err(err) = self.predict_message_impl(ctx, msg).await {
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

    let config: BotConfig = toml::from_slice(
        fs::read("bot.toml")
            .await
            .expect("failed to read bot.toml")
            .as_slice(),
    )
    .expect("failed to parse bot.toml");

    let replicate_token =
        env::var("REPLICATE_TOKEN").expect("REPLICATE_TOKEN environment variable must be set");
    let discord_token =
        env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN environment variable must be set");

    let allowed_channel_ids = HashSet::from_iter(config.allowed_channels.iter().copied());
    let handler = Handler {
        allowed_channel_ids,
        sd_api: StableDiffusionApi::new(&replicate_token),
    };

    let mut client = Client::builder(
        &discord_token,
        GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILD_MESSAGES,
    )
    .cache_settings(|settings| settings.max_messages(100))
    .event_handler(handler)
    .await
    .expect("error creating client");

    client.start().await.expect("error starting client");
}
