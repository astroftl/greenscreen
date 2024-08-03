mod discord;
mod commands;

#[macro_use]
extern crate log;

use std::env;

use fern::colors::{Color, ColoredLevelConfig};
use serenity::all::ApplicationId;
use serenity::Client;
use serenity::prelude::GatewayIntents;
use songbird::{Config, SerenityInit};
use songbird::driver::DecodeMode;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let bot_token = env::var("BOT_TOKEN").expect("Expected a BOT_TOKEN in the environment");

    let app_id: ApplicationId = env::var("APP_ID").expect("Expected an APP_ID in the environment")
        .parse().expect("APP_ID is not a valid ID");

    setup_logger();

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;

    // Here, we need to configure Songbird to decode all incoming voice packets.
    // If you want, you can do this on a per-call basis---here, we need it to
    // read the audio data that other people are sending us!
    let songbird_config = Config::default().decode_mode(DecodeMode::Decode);

    let mut client = Client::builder(&bot_token, intents)
        .event_handler(discord::Events)
        .application_id(app_id)
        .register_songbird_from_config(songbird_config)
        .await
        .expect("Error creating client");

    info!("Starting Greenscreen...");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    info!("Goodbye!")
}

fn setup_logger() {
    let colors_line = ColoredLevelConfig::new()
        .error(Color::BrightRed)
        .warn(Color::BrightYellow)
        .info(Color::BrightWhite)
        .debug(Color::White)
        .trace(Color::BrightBlack);

    let colors_level = colors_line.clone()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::BrightGreen)
        .debug(Color::BrightCyan)
        .trace(Color::Black);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        .level(log::LevelFilter::Warn)
        .level_for("dota_stalker", log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(fern::log_file("stalker.log").unwrap())
        .apply()
        .unwrap();
}