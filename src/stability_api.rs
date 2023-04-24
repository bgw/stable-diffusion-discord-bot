use std::borrow::Cow;

use anyhow::anyhow;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Deserializer, Serialize};

const ENGINE_ID: &str = "stable-diffusion-xl-beta-v2-2-2";
const STABILITY_API_URI: &str = "https://api.stability.ai/v1";

pub struct StabilityApi {
    reqwest_client: reqwest::Client,
}

impl StabilityApi {
    pub fn new(stability_api_key: &str) -> Self {
        let mut replicate_headers = HeaderMap::new();
        replicate_headers.append(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {stability_api_key}"))
                .expect("failed to construct authorization header"),
        );
        replicate_headers.append(header::EXPECT, HeaderValue::from_static("application/json"));
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

    pub async fn text_to_image(
        &self,
        input: &TextToImageRequest<'_>,
    ) -> anyhow::Result<Vec<TextToImageResponse>> {
        #[derive(Deserialize)]
        struct ArtifactsResponse {
            artifacts: Vec<TextToImageResponse>,
        }

        let response = self
            .reqwest_client
            .post(format!(
                "{STABILITY_API_URI}/generation/{ENGINE_ID}/text-to-image"
            ))
            .json(&input)
            .send()
            .await?;

        match response.status().as_u16() {
            200 => {
                let artifacts_response: ArtifactsResponse = response.json().await?;
                Ok(artifacts_response.artifacts)
            }
            400..=599 => {
                let err: ErrorResponse = response.json().await?;
                Err(anyhow!("Got an error from the backend: {}", err.message))
            }
            _ => Err(anyhow!(
                "Got an unknown HTTP status code from the backend: {}",
                response.status()
            )),
        }
    }
}

#[derive(Serialize)]
pub struct TextPrompt<'a> {
    pub text: Cow<'a, str>,
    pub weight: Option<f64>,
}

#[derive(Default, Serialize)]
pub struct TextToImageRequest<'a> {
    /// default 512, multiple of 64
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,

    /// default 512, multiple of 64
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u64>,

    pub text_prompts: Vec<TextPrompt<'a>>,

    /// 0..=35, default 7
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_guidance_preset: Option<&'a str>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampler: Option<&'a str>,

    /// number of images to generate (1..=10), default 1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples: Option<u64>,

    /// default random, any u32 is valid, 0 means "random"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,

    /// 10..=150, default 50
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_preset: Option<&'a str>,
}

#[derive(Deserialize)]
pub struct TextToImageResponse {
    #[serde(rename = "base64", deserialize_with = "deserialize_base64")]
    pub image: Vec<u8>,
    #[serde(rename = "finishReason")]
    pub finish_reason: String,
    pub seed: u32,
}

#[derive(Deserialize)]
pub struct ErrorResponse {
    pub id: String,
    pub name: String,
    pub message: String,
}

const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    base64::engine::GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
);

fn deserialize_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use base64::Engine;

    String::deserialize(deserializer).and_then(|string| {
        BASE64_ENGINE
            .decode(&string)
            .map_err(|err| serde::de::Error::custom(err.to_string()))
    })
}
