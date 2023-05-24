pub use crate::app::library::Library;
pub use crate::app::player::Player;
pub use crate::app::App;
pub use crate::app::*;
use clap::Arg;
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;

mod app;

pub enum PlayerState {
    Unstarted,
    Stopped,
    Playing,
    Paused,
}

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("starting app");

    let matches = clap::Command::new("Drummer's Karaoke")
        .version("0.1")
        .arg(
            Arg::new("music_folder")
                .long("music_folder")
                .required(false)
                .help("Where your music files are stored"),
        )
        .arg(
            Arg::new("song_file")
                .long("song_file")
                .required(false)
                .help("The CSV file with the song list"),
        )
        .get_matches();

    let music_folder = matches
        .get_one::<String>("music_folder")
        .map(|s| s.to_owned());

    let song_file = matches.get_one::<String>("song_file").map(|s| s.to_owned());

    let initial_library = match music_folder {
        Some(folder) => {
            tracing::info!("Will use music from: {}", folder);
            let path = PathBuf::from(folder);

            let mut library = Library::new(path);

            if let Some(song_file) = song_file {
                let song_file = PathBuf::from(song_file);
                library.load_songs(song_file);
            }

            Some(library)
        }
        None => {
            tracing::info!("No default music folder was provided.");
            None
        }
    };
    let (audio_tx, audio_rx) = channel();
    let (tx, rx) = channel();
    let cursor = Arc::new(AtomicU32::new(0));
    let _cursor_clone = cursor.clone();
    let player = Player::new(audio_tx, cursor);

    let mut app = App::load().unwrap_or_default();
    app.player = Some(player);
    app.library = initial_library;
    app.library_sender = Some(tx);
    app.library_receiver = Some(rx);

    let _audio_thread = thread::spawn(move || loop {
        let state = Arc::new(Mutex::new(PlayerState::Stopped));
        let state_clone = state.clone();

        match audio_rx.try_recv() {
            Ok(cmd) => match cmd {
                AudioCommand::PlayTrack(track) => {
                    tracing::info!("Audio will load the song: {}", track.title().unwrap());
                }
                AudioCommand::Play => {
                    tracing::info!("Audio will play the song");
                    let mut guard = state.lock().unwrap();
                    *guard = PlayerState::Playing;
                }
                AudioCommand::Pause => {
                    tracing::info!("Audio will pause the song");
                    let mut guard = state.lock().unwrap();
                    *guard = PlayerState::Paused;
                }
                AudioCommand::Stop => {
                    tracing::info!("Audio will stop the song");
                    let mut guard = state.lock().unwrap();
                    *guard = PlayerState::Stopped;
                }
                _ => {
                    tracing::info!("Audio does not implement the requested event");
                }
            },
            _ => {} // throw away bad events
        }
    });

    let mut window_options = eframe::NativeOptions::default();
    window_options.initial_window_size = Some(egui::Vec2::new(1024., 768.));
    eframe::run_native(
        "Drum Karaoke Player",
        window_options,
        Box::new(|_| Box::new(app)),
    )
    .expect("eframe failed: I should change main to return a result and use anyhow");
}
