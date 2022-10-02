use std::collections::HashSet;
use std::env;
use std::time::Duration;

use anyhow::Context as AnyhowContext;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::model::channel::Message;
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::ChannelId;
use tokio::time::sleep;
use tracing::{debug, error, info, instrument};

const STABLE_DIFFUSION_VERSION: &str =
    "a9758cbfbd5f3c2094457d996681af52552901775aa2d6dd0b17fd15df959bef";
const REPLICATE_API_URI: &str = "https://api.replicate.com/v1";

struct StableDiffusionApi {
    reqwest_client: reqwest::Client,
}

#[derive(Default, Serialize)]
struct StableDiffusionRequest<'a> {
    prompt: &'a str,
    /// Width of output image. Maximum size is 1024x768 or 768x1024 because of memory limits
    width: Option<u32>,
    /// Height of output image. Maximum size is 1024x768 or 768x1024 because of memory limits
    height: Option<u32>,
    /// Number of denoising steps (minimum: 1; maximum: 500)
    num_inference_steps: Option<u32>,
    /// Scale for classifier-free guidance (minimum: 1; maximum: 20)
    guidance_scale: Option<f32>,
    /// Number of images to output
    num_outputs: Option<u32>,
}

impl StableDiffusionApi {
    async fn predict<In, Out>(&self, version: &str, input: In) -> anyhow::Result<Out>
    where
        In: Serialize,
        Out: DeserializeOwned,
    {
        #[derive(Serialize)]
        struct CreatePredictionRequest<'a, T> {
            version: &'a str,
            input: T,
        }

        #[derive(Deserialize)]
        struct PredictionResponse<T> {
            id: String,
            status: String,
            output: Option<T>,
        }

        let mut response: PredictionResponse<Out> = self
            .reqwest_client
            .post(format!("{REPLICATE_API_URI}/predictions"))
            .json(&CreatePredictionRequest { version, input })
            .send()
            .await?
            .json()
            .await?;

        let prediction_id = response.id;

        while matches!(&*response.status, "starting" | "processing") {
            response = self
                .reqwest_client
                .get(format!("{REPLICATE_API_URI}/predictions/{prediction_id}"))
                .send()
                .await?
                .json()
                .await?;
            sleep(Duration::from_millis(500)).await;
        }

        response.output.context("request to backend failed!")
    }
}

struct Handler {
    allowed_channel_ids: HashSet<ChannelId>,
    sd_api: StableDiffusionApi,
}

impl Handler {
    async fn predict_message_impl(&self, ctx: Context, msg: Message) -> anyhow::Result<()> {
        debug!(%msg.author, %msg.content, "received message");
        let mut reply_msg = msg.reply(&ctx, "*Processing...*").await?;

        let img_uri_result = async {
            let mut uri_vec: Vec<String> = self
                .sd_api
                .predict(
                    STABLE_DIFFUSION_VERSION,
                    &StableDiffusionRequest {
                        prompt: &msg.content,
                        width: Some(512),
                        height: Some(512),
                        num_inference_steps: Some(30),
                        guidance_scale: Some(7.5),
                        num_outputs: Some(1),
                    },
                )
                .await?;
            uri_vec
                .pop()
                .context("expected at least one result from replicate")
        }
        .await;

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
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let replicate_token =
        env::var("REPLICATE_TOKEN").expect("REPLICATE_TOKEN environment variable must be set");
    let discord_token =
        env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN environment variable must be set");
    let channel_id: u64 = env::var("CHANNEL_ID")
        .expect("CHANNEL_ID environment variable must be set")
        .parse()
        .expect("CHANNEL_ID must be a valid u64");

    let mut replicate_headers = HeaderMap::new();
    replicate_headers.append(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Token {replicate_token}"))
            .expect("failed to construct authorization header"),
    );
    replicate_headers.append(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );

    let mut allowed_channel_ids = HashSet::new();
    allowed_channel_ids.insert(channel_id.into());
    let handler = Handler {
        allowed_channel_ids,
        sd_api: StableDiffusionApi {
            reqwest_client: reqwest::Client::builder()
                .default_headers(replicate_headers)
                .build()
                .expect("failed to create reqwest client"),
        },
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
