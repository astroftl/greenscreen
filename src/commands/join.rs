use serenity::all::{CommandInteraction, Context, InteractionContext};
use serenity::builder::CreateCommand;
use songbird::CoreEvent;

pub const NAME: &str = "join";

pub async fn run(ctx: &Context, cmd: &CommandInteraction) {
    let user = cmd.clone().member.unwrap();
    let channels = cmd.guild_id.unwrap().channels(ctx).await.unwrap();
    for (channel_id, guild_channel) in channels {
        let members = guild_channel.members(ctx).unwrap();
        let member = members.iter().find(|x| {
            x.user.id == user.user.id
        });
        if member.is_some() {
            let manager = songbird::get(ctx)
                .await
                .expect("Songbird Voice client placed in at initialisation.")
                .clone();

            if let Ok(handler_lock) = manager.join(cmd.guild_id, channel_id).await {
                // NOTE: this skips listening for the actual connection result.
                let mut handler = handler_lock.lock().await;

                let evt_receiver = crate::discord::Receiver::new();

                handler.add_global_event(CoreEvent::SpeakingStateUpdate.into(), evt_receiver.clone());
                handler.add_global_event(CoreEvent::RtpPacket.into(), evt_receiver.clone());
                handler.add_global_event(CoreEvent::RtcpPacket.into(), evt_receiver.clone());
                handler.add_global_event(CoreEvent::ClientDisconnect.into(), evt_receiver.clone());
                handler.add_global_event(CoreEvent::VoiceTick.into(), evt_receiver);

                crate::discord::check_msg(
                    msg.channel_id
                        .say(&ctx.http, &format!("Joined {}", connect_to.mention()))
                        .await,
                );
            } else {
                crate::discord::check_msg(
                    msg.channel_id
                        .say(&ctx.http, "Error joining the channel")
                        .await,
                );
            }
        }
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description("Get a user id")
        .add_context(InteractionContext::Guild)
}