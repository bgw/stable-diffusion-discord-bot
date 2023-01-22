use std::borrow::Cow;
use std::time::Duration;

use anyhow::Context as AnyhowContext;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

pub const STABLE_DIFFUSION_VERSION: &str =
    "f178fa7a1ae43a9a9af01b833b9d2ecf97b1bcb0acfd2dc5dd04895e042863f1";
const REPLICATE_API_URI: &str = "https://api.replicate.com/v1";

pub struct StableDiffusionApi {
    reqwest_client: reqwest::Client,
}

impl StableDiffusionApi {
    pub fn new(replicate_token: &str) -> Self {
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

        Self {
            reqwest_client: reqwest::Client::builder()
                .default_headers(replicate_headers)
                .build()
                .expect("failed to create reqwest client"),
        }
    }

    pub async fn predict<In, Out>(&self, version: &str, input: In) -> anyhow::Result<Out>
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
            #[serde(default)]
            error: Option<String>,
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

        if let Some(err) = response.error {
            anyhow::bail!("Got an error from the backend: {err}");
        }

        response
            .output
            .context("Did not get an output from the backend!")
    }
}

#[derive(Default, Serialize)]
pub struct StableDiffusionRequest<'a> {
    pub prompt: Cow<'a, str>,
    /// Width of output image. Maximum size is 1024x768 or 768x1024 because of memory limits
    pub width: Option<u32>,
    /// Height of output image. Maximum size is 1024x768 or 768x1024 because of memory limits
    pub height: Option<u32>,
    /// Number of denoising steps (minimum: 1; maximum: 500)
    pub num_inference_steps: Option<u32>,
    /// Scale for classifier-free guidance (minimum: 1; maximum: 20)
    pub guidance_scale: Option<f32>,
    /// Number of images to output
    pub num_outputs: Option<u32>,
}
