use once_cell::sync::Lazy;
use regex::Regex;

use crate::replicate_api::StableDiffusionRequest;

static MODIFIER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new("![a-zA-Z_]+\\b").expect("regex compilation"));

pub fn prompt_parse(msg: &str) -> anyhow::Result<StableDiffusionRequest<'_>> {
    let mut request = StableDiffusionRequest {
        prompt: MODIFIER_RE.replace_all(msg, ""),
        width: Some(512),
        height: Some(512),
        num_inference_steps: Some(30),
        guidance_scale: Some(7.5),
        num_outputs: Some(1),
    };

    for modifier in MODIFIER_RE.find_iter(msg) {
        match modifier.as_str() {
            "!quality" => {
                request.num_inference_steps = Some(80);
            }
            "!strict" => {
                request.guidance_scale = Some(15.0);
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
