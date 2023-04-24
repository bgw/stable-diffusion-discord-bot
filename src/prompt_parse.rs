use once_cell::sync::Lazy;
use regex::Regex;

use crate::stability_api::{TextPrompt, TextToImageRequest};

static MODIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("![a-zA-Z_]+\\b").expect("regex compilation"));

pub fn prompt_parse(msg: &str) -> anyhow::Result<TextToImageRequest<'_>> {
    let mut request = TextToImageRequest {
        height: Some(512),
        width: Some(512),
        text_prompts: vec![TextPrompt {
            text: MODIFIER_RE.replace_all(msg, ""),
            weight: None,
        }],
        ..Default::default()
    };

    for modifier in MODIFIER_RE.find_iter(msg) {
        match modifier.as_str() {
            "!quality" => {
                request.steps = Some(100);
            }
            "!strict" => {
                request.cfg_scale = Some(15.0);
            }
            "!large" => {
                request.width = Some(768);
                request.height = Some(768);
            }
            mod_str => {
                anyhow::bail!("{mod_str} is not a supported modifier (!quality, !strict, !large)");
            }
        }
    }

    Ok(request)
}
