use anyhow::Context;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::stability_api::TextToImageRequest;

static MODIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("!([a-zA-Z_-]+)\\b").expect("regex compilation"));

pub fn prompt_parse(msg: &str) -> anyhow::Result<TextToImageRequest> {
    let request = TextToImageRequest {
        prompt: msg.to_owned(),
        ..Default::default()
    };

    for capture in MODIFIER_RE.captures_iter(msg) {
        let modifier = capture
            .get(1)
            .context("regex capture should contain group 1")?;
        let modifier = modifier.as_str();
        anyhow::bail!(
            "modifiers ({modifier}) are no longer supported, they may be added back in the future"
        );
    }

    Ok(request)
}
