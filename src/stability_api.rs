use anyhow::{anyhow, Context};
use bytes::Bytes;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::multipart::Form;
use serde::Serialize;

const STABILITY_API_URI: &str = "https://api.stability.ai/v2beta";

pub struct StabilityApi {
    reqwest_client: reqwest::Client,
}

impl StabilityApi {
    pub fn new(stability_api_key: &str) -> Self {
        let mut stability_headers = HeaderMap::new();
        stability_headers.append(
            header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {stability_api_key}"))
                .expect("failed to construct authorization header"),
        );
        stability_headers.append(header::ACCEPT, HeaderValue::from_static("image/*"));
        stability_headers.append(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data"),
        );

        Self {
            reqwest_client: reqwest::Client::builder()
                .default_headers(stability_headers)
                .build()
                .expect("failed to create reqwest client"),
        }
    }

    pub async fn text_to_image(
        &self,
        input: TextToImageRequest,
    ) -> anyhow::Result<TextToImageResponse> {
        let mut form = Form::new().text("prompt", input.prompt.to_owned());

        if let Some(ar) = input.aspect_ratio {
            form = form.text("aspect_ratio", ar);
        }

        if let Some(np) = input.negative_prompt {
            form = form.text("negative_prompt", np);
        }

        if let Some(md) = input.model {
            form = form.text("model", md);
        }

        if let Some(sd) = input.seed {
            form = form.text("seed", sd.to_string());
        }

        let response = self
            .reqwest_client
            .post(format!("{STABILITY_API_URI}/stable-image/generate/sd3"))
            .multipart(form)
            .send()
            .await?;

        match response.status().as_u16() {
            200 => Ok(TextToImageResponse {
                image: response
                    .bytes()
                    .await
                    .context("Failed to read full response")?,
            }),
            400..=599 => {
                let err = response.text().await?;
                Err(anyhow!("Got an error from the backend: {}", err))
            }
            _ => Err(anyhow!(
                "Got an unknown HTTP status code from the backend: {}",
                response.status()
            )),
        }
    }
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct TextToImageRequest {
    pub prompt: String,
    pub aspect_ratio: Option<String>,
    pub negative_prompt: Option<String>,
    pub model: Option<String>,
    pub seed: Option<u32>,
}

pub struct TextToImageResponse {
    pub image: Bytes,
}
