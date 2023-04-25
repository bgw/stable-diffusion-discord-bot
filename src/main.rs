use std::collections::HashSet;
use std::env;

use prompt_parse::prompt_parse;
use serde::Deserialize;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::channel::{AttachmentType, Message};
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
    async fn predict_message_impl(&self, ctx: Context, msg: Message) -> anyhow::Result<()> {
        debug!(%msg.author, %msg.content, "received message");

        let request = match prompt_parse(&msg.content) {
            Ok(req) => req,
            Err(err) => {
                msg.reply(&ctx, &err).await?;
                return Err(err);
            }
        };

        let (reply_msg, responses_result) =
            tokio::join!(msg.reply(&ctx, "*Processing...*"), async {
                self.stability_api.text_to_image(&request).await
            });

        let mut reply_msg = reply_msg?;

        let responses = match responses_result {
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
            .edit(ctx, move |edit_msg| {
                edit_msg.content("");
                for resp in responses {
                    edit_msg.attachment(AttachmentType::Bytes {
                        data: resp.image.into(),
                        filename: format!("stablediffusionbot.png"),
                    });
                }
                edit_msg
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

    let stability_api_key =
        env::var("STABILITY_API_KEY").expect("STABILITY_API_KEY environment variable must be set");
    let discord_token =
        env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN environment variable must be set");

    let allowed_channel_ids = HashSet::from_iter(config.allowed_channels.iter().copied());
    let handler = Handler {
        allowed_channel_ids,
        stability_api: StabilityApi::new(&stability_api_key),
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
