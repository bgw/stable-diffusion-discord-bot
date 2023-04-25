use anyhow::Context;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

use crate::stability_api::{TextPrompt, TextToImageRequest};

static MODIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("!([a-zA-Z_-]+)\\b").expect("regex compilation"));

static STYLE_PRESETS_VEC: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        "3d-model",
        "analog-film",
        "anime",
        "cinematic",
        "comic-book",
        "digital-art",
        "enhance",
        "fantasy-art",
        "isometric",
        "line-art",
        "low-poly",
        "modeling-compound",
        "neon-punk",
        "origami",
        "photographic",
        "pixel-art",
        "tile-texture",
    ]
});

static STYLE_PRESETS_SET: Lazy<HashSet<&'static str>> =
    Lazy::new(|| STYLE_PRESETS_VEC.iter().copied().collect());

static STYLE_PRESETS_STRING: Lazy<String> = Lazy::new(|| {
    STYLE_PRESETS_VEC
        .iter()
        .map(|preset| format!("!{preset}"))
        .collect::<Vec<_>>()
        .join(", ")
});

pub fn prompt_parse(msg: &str) -> anyhow::Result<TextToImageRequest<'_>> {
    let mut request = TextToImageRequest {
        height: Some(512),
        width: Some(512),
        text_prompts: vec![TextPrompt {
            text: MODIFIER_RE.replace_all(msg, ""),
            weight: None,
        }],
        samples: Some(3),
        ..Default::default()
    };

    for capture in MODIFIER_RE.captures_iter(msg) {
        let modifier = capture
            .get(1)
            .context("regex capture should contain group 1")?;
        let modifier = modifier.as_str();
        if let Some(style_preset) = STYLE_PRESETS_SET.get(modifier) {
            if request.style_preset.is_some() {
                anyhow::bail!("only one style preset is allowed at once");
            }
            request.style_preset = Some(style_preset); // use the 'static version
        } else {
            match modifier {
                "quality" => {
                    request.samples = Some(1);
                    request.steps = Some(100);
                }
                "strict" => {
                    request.cfg_scale = Some(15.0);
                }
                mod_str => {
                    anyhow::bail!(
                        "{mod_str} is not a supported modifier (!quality, !strict) \
                        or style preset ({})",
                        *STYLE_PRESETS_STRING,
                    );
                }
            }
        }
    }

    Ok(request)
}
