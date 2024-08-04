use serenity::all::{ChannelId, CommandInteraction, Context, CreateInteractionResponseMessage, InteractionContext};
use serenity::builder::{CreateCommand, CreateInteractionResponse};

pub const NAME: &str = "leave";

pub async fn run(ctx: &Context, cmd: &CommandInteraction) {
    let guild_id = cmd.guild_id.unwrap();

    let manager = songbird::get(ctx)
        .await
        .expect("Songbird Voice client placed in at initialisation.")
        .clone();

    let call = manager.get(guild_id);

    if let Some(call) = call {
        let channel_id = ChannelId::from(call.lock().await.current_channel().unwrap().0);
        let channel_name = channel_id.name(ctx).await.unwrap();

        call.lock().await.remove_all_global_events();
        if let Err(e) = manager.remove(guild_id).await {
            let resp = CreateInteractionResponseMessage::new()
                .content(format!("Failed: {e:?}"))
                .ephemeral(true);

            cmd.create_response(ctx, CreateInteractionResponse::Message(resp)).await.unwrap();
        } else {
            let resp = CreateInteractionResponseMessage::new()
                .content(format!("Left \"{channel_name}\""))
                .ephemeral(true);

            cmd.create_response(ctx, CreateInteractionResponse::Message(resp)).await.unwrap();
        }
    } else {
        let resp = CreateInteractionResponseMessage::new()
            .content("Not in a voice channel!")
            .ephemeral(true);

        cmd.create_response(ctx, CreateInteractionResponse::Message(resp)).await.unwrap();
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description("Leave a voice channel")
        .add_context(InteractionContext::Guild)
}