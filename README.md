# Stable Diffusion Discord Bot

Listens to messages on a Discord channel, and sends those messages to 
[Replicate's HTTP API][replicate http], to be run through [Stable Diffusion].

Similar to [this Python bot][python bot], but in Rust with [Serenity][].

[replicate http]: https://replicate.com/docs/reference/http
[Stable Diffusion]: https://replicate.com/stability-ai/stable-diffusion
[python bot]: https://replicate.com/blog/build-a-robot-artist-for-your-discord-server-with-stable-diffusion
[Serenity]: https://docs.rs/serenity/latest/serenity/

## Usage

Modify `bot.toml` and add allowed channel ids.

```
REPLICATE_TOKEN=... DISCORD_TOKEN=... cargo run
```

## Notes

Make sure to set a spend limit on your Replicate account!
