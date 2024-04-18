# Stable Diffusion Discord Bot

Listens to messages on a Discord channel, and sends those messages to 
[Stability AI's HTTP API][sdapi], to be run through Stable Diffusion.

Similar to [this Python bot][python bot], but in Rust with [Serenity][].

[sdapi]: https://platform.stability.ai/docs/api-reference
[python bot]: https://replicate.com/blog/build-a-robot-artist-for-your-discord-server-with-stable-diffusion
[Serenity]: https://docs.rs/serenity/latest/serenity/

## Usage

Modify `bot.toml` and add allowed channel ids.

```
STABILITY_API_KEY=... DISCORD_TOKEN=... cargo run
```

## Notes

Make sure to set a spend limit on your Replicate account!
