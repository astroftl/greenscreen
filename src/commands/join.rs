use serenity::all::{ChannelType, CommandInteraction, CommandOptionType, Context, CreateCommandOption, InteractionContext};
use serenity::builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage};
use songbird::CoreEvent;
use crate::discord::DiscordData;
use crate::voice_handler::Receiver;

pub const NAME: &str = "join";

pub async fn run(ctx: &Context, cmd: &CommandInteraction) {
    let guild_id = cmd.guild_id.unwrap();
    let channel_id = cmd.data.options.first().unwrap().value.as_channel_id().unwrap();

    debug!("Joining: {channel_id:?} @ {guild_id:?}");

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialization.")
        .clone();

    if let Ok(handler_lock) = manager.join(guild_id, channel_id).await {
        // NOTE: this skips listening for the actual connection result.
        let mut handler = handler_lock.lock().await;

        let ws_serv = ctx.data.read().await.get::<DiscordData>().unwrap().ws_server.clone();
        let event_tx  = ws_serv.event_tx.clone();

        let evt_receiver = Receiver::new(event_tx, guild_id).await;

        handler.add_global_event(CoreEvent::SpeakingStateUpdate.into(), evt_receiver.clone());
        handler.add_global_event(CoreEvent::ClientDisconnect.into(), evt_receiver.clone());
        handler.add_global_event(CoreEvent::VoiceTick.into(), evt_receiver);

        let channel_name = channel_id.name(ctx).await.unwrap_or_else(|e| {
            error!("Error retrieving channel name: {e:?}");
            String::new()
        });

        let resp = CreateInteractionResponseMessage::new()
            .content(format!("Joined \"{channel_name}\"\nWebSocket server is available at: ws://greenscreen.ftl.sh/{guild_id}"))
            .ephemeral(true);

        cmd.create_response(ctx, CreateInteractionResponse::Message(resp)).await.unwrap_or_else(|e| {
            error!("Error responding to the interaction: {e:?}");
        });
    } else {
        let resp = CreateInteractionResponseMessage::new()
            .content("Failed to join channel!")
            .ephemeral(true);

        cmd.create_response(ctx, CreateInteractionResponse::Message(resp)).await.unwrap_or_else(|e| {
            error!("Error responding to the interaction: {e:?}");
        });
    }

    trace!("Left run()");
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description("Join a voice channel")
        .add_context(InteractionContext::Guild)
        .add_option(
            CreateCommandOption::new(CommandOptionType::Channel, "channel", "Voice channel to join")
                .required(true)
                .channel_types(vec![ChannelType::Voice])
        )
}