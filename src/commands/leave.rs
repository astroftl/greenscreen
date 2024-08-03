use serenity::all::{CommandInteraction, Context, InteractionContext};
use serenity::builder::CreateCommand;

pub const NAME: &str = "leave";

pub fn run(ctx: &Context, cmd: &CommandInteraction) {
    // TODO: leave the VC
}

pub fn register() -> CreateCommand {
    CreateCommand::new(NAME)
        .description("Get a user id")
        .add_context(InteractionContext::Guild)
}