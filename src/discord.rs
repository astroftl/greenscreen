use std::sync::Arc;
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::gateway::Ready,
};
use serenity::all::{Command, Interaction};
use serenity::prelude::TypeMapKey;

use crate::commands;
use crate::ws_server::WebsocketServer;

pub struct DiscordData {
    pub ws_server: Arc<WebsocketServer>,
}

impl TypeMapKey for DiscordData {
    type Value = DiscordData;
}

pub struct Events;

#[async_trait]
impl EventHandler for Events {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
        Command::set_global_commands(&ctx.http,
            vec![
                commands::join::register(),
                commands::leave::register(),
            ]
        ).await.expect("Failed to register global commands!");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            match command.data.name.as_str() {
                commands::join::NAME => commands::join::run(&ctx, &command).await,
                commands::leave::NAME => commands::leave::run(&ctx, &command).await,
                _ => {}
            }
        }
    }
}