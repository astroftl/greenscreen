# Greenscreen
Greenscreen is a Discord bot which listens for connects, disconnects, and voice activity (and only activity, not actual voice data) in Discord Voice Channels and reports these events over a WebSocket stream.

The intended use of this is for custom stream overlays (see [demihumans-greenscreen](https://github.com/astroftl/demihumans-greenscreen) for a simple example) which display active speaking users in a simple webpage suitable for use as an OBS browser source.

### Usage
Add the bot to a Discord server using [this link.](https://discord.com/oauth2/authorize?client_id=1261493113894207508)

The bot has two commands, `/join <channel name>` which joins the bot to a voice channel, and `/leave` which commands the bot to leave the voice channel it's in, if any. Note that the bot must have permission to join a given channel, and that channels which you can see may not be visible to the bot by default.

Once the bot is connected, it will begin sending connect, disconnect, start speaking, and stop speaking events via a WebSocket server located at `wss://greenscreen.ftl.sh/<guild_id>`.

These events are in JSON format, one event per message. The full messages are:

```json
{"connected":"<user_id>"}

{"disconnected":"<user_id>"}

{"speaking":"<user_id>"}

{"quiet":"<user_id>"}
```

Since these events are only sent when necessary and there is no persistent state, there is also a heartbeat message consisting only of `"heartbeat"` every 10 seconds as a form of keep-alive.

Note that the `connected` event actually fires on the first instance of someone speaking, not the moment they join the voice channel. Likewise, a `disconnected` event will only fire if a `connected` event has first been sent, so someone who joins a call, says nothing, and then leaves will have no events sent for them.