use std::{
    env,
    error::Error,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::{self, current},
    time::Duration,
};

use cpal::traits::HostTrait;
use crossbeam_channel::{Receiver, Sender};
use log::{error, info};

use crate::app::audio::Song;

use super::{audio::AudioPlayer, library::SongRecord};
pub struct Player {
    player_command_receiver: Receiver<PlayerCommand>,
    player_event_sender: Sender<PlayerEvent>,
    is_paused: Arc<Mutex<bool>>,
}

#[derive(Debug, Clone)]
pub struct SongStub {
    pub file_name: String,
    pub title: String,
    pub folder: String,
}

impl SongStub {
    pub fn from_song_record(song_record: &SongRecord) -> Self {
        SongStub {
            file_name: song_record.file_name.clone(),
            title: song_record.title.clone(),
            folder: song_record.folder.clone(),
        }
    }
}
#[derive(Debug)]
pub enum PlayerCommand {
    Play(SongStub),
    Pause,
    Stop,
    Quit,
    GetPosition,
}

#[derive(Debug)]
pub enum PlayerEvent {
    Playing(SongStub),
    LoadFailure(SongStub),
    Position(Option<(Duration, Duration)>),
    Paused,
    Continuing(Option<SongStub>),
    Stopped,
    Decompressing,
    Decompressed,
    Quit,
}

impl Player {
    pub fn new(player_command_receiver: Receiver<PlayerCommand>, player_event_sender: Sender<PlayerEvent>) -> Self {
        Player {
            player_command_receiver,
            player_event_sender,
            is_paused: Arc::new(Mutex::new(false)),
        }
    }

    pub fn run(&mut self) {
        let is_paused = self.is_paused.clone();

        let player_event_sender = self.player_event_sender.clone();
        let player_command_receiver = self.player_command_receiver.clone();

        thread::spawn(move || {
            let host = cpal::default_host();
            let available_devices = host.output_devices().unwrap().collect::<Vec<_>>();

            // TODO: Devices from configuration
            let track_device = &available_devices[0];
            let click_device = &available_devices[0];

            let track_player = AudioPlayer::new(None, track_device).expect("Could not create track player");
            let click_player = AudioPlayer::new(None, click_device).expect("Could not create click player");
            track_player.set_playback_speed(1.0);
            click_player.set_playback_speed(1.0);

            let mut current_stub: Option<SongStub> = None;

            loop {
                // See if any commands have been sent to the player
                match player_command_receiver.try_recv() {
                    Ok(command) => match command {
                        PlayerCommand::Play(stub) => {
                            //info!("Player will load: {:?}", stub.file_name);4
                            // 1. Check if the wav and click files exist. If not, decompress from the 7z file
                            // 2. Load the wav and click files into the player
                            // 3. Play the song
                            let (track_path, click_path) = Self::get_file_paths(stub.folder.as_str(), stub.file_name.as_str());
                            track_player.set_playing(false);
                            click_player.set_playing(false);

                            if !track_path.exists() {
                                info!("Track file does not exist. Decompressing.");
                                player_event_sender.send(PlayerEvent::Decompressing).unwrap();
                                thread::sleep(std::time::Duration::from_millis(1000));

                                match Self::decompress_files(stub.folder.as_str(), stub.file_name.as_str()) {
                                    Ok(()) => {
                                        info!("Decompression complete");
                                        player_event_sender.send(PlayerEvent::Decompressed).unwrap();
                                        thread::sleep(std::time::Duration::from_millis(1000));
                                    }
                                    Err(err) => {
                                        info!("Decompression failed: {:?}", err);
                                        player_event_sender.send(PlayerEvent::LoadFailure(stub.clone())).unwrap();
                                        thread::sleep(std::time::Duration::from_millis(1000));
                                        continue;
                                    }
                                }
                            }

                            let track_song = Song::from_file(track_path.clone(), None);
                            let click_song = Song::from_file(click_path.clone(), None);

                            if let Err(err) = track_song {
                                error!("Failed to load song: {:?}", err);
                                player_event_sender.send(PlayerEvent::LoadFailure(stub.clone())).unwrap();
                                current_stub = None;
                                continue;
                            }

                            if let Err(err) = click_song {
                                error!("Failed to load click: {:?}", err);
                                player_event_sender.send(PlayerEvent::LoadFailure(stub.clone())).unwrap();
                                current_stub = None;
                                continue;
                            }

                            let track_song = track_song.unwrap();
                            let click_song = click_song.unwrap();

                            track_player.stop();
                            click_player.stop();

                            let track_status = track_player.play_song_now(&track_song, None);
                            let click_status = click_player.play_song_now(&click_song, None);

                            match (track_status, click_status) {
                                (Ok(_), Ok(_)) => {
                                    // Both track and click songs were played successfully
                                    track_player.set_playing(true);
                                    click_player.set_playing(true);
                                    current_stub = Some(stub.clone());

                                    player_event_sender.send(PlayerEvent::Playing(stub.clone())).unwrap();
                                }
                                (Err(track_err), _) => {
                                    // Failed to play the track song
                                    error!("Failed to play track song: {:?}", track_err);
                                    player_event_sender.send(PlayerEvent::LoadFailure(stub.clone())).unwrap();
                                    track_player.stop();
                                    click_player.stop();
                                    current_stub = None;
                                    continue;
                                }
                                (_, Err(click_err)) => {
                                    // Failed to play the click song
                                    error!("Failed to play click song: {:?}", click_err);
                                    player_event_sender.send(PlayerEvent::LoadFailure(stub.clone())).unwrap();
                                    track_player.stop();
                                    click_player.stop();
                                    current_stub = None;
                                    continue;
                                }
                            }
                        }
                        PlayerCommand::Pause => {
                            let is_playing = track_player.is_playing();

                            track_player.set_playing(!is_playing);
                            click_player.set_playing(!is_playing);

                            if !is_playing {
                                info!("Player is continuing");
                                player_event_sender.send(PlayerEvent::Continuing(current_stub.clone())).unwrap();
                            } else {
                                info!("Player is pausing");
                                player_event_sender.send(PlayerEvent::Paused).unwrap();
                            }
                        }
                        PlayerCommand::Stop => {
                            info!("Player will stop");
                            player_event_sender.send(PlayerEvent::Stopped).unwrap();
                        }
                        PlayerCommand::Quit => {
                            info!("Player received quit signal. Exiting.");
                            track_player.stop();
                            click_player.stop();
                            player_event_sender.send(PlayerEvent::Quit).unwrap();
                            thread::sleep(std::time::Duration::from_millis(100)); // time for the exit to propagate
                            break;
                        }
                        PlayerCommand::GetPosition => {
                            if let Some((position, duration)) = track_player.get_playback_position() {
                                if track_player.is_playing() {
                                    player_event_sender.send(PlayerEvent::Position(Some((position, duration)))).unwrap();
                                } else {
                                    player_event_sender.send(PlayerEvent::Position(None)).unwrap();
                                }
                            }
                        }
                    },
                    Err(_err) => {}
                }
            }
        });
    }

    fn get_beep_file() -> String {
        let mut path = env::current_dir().expect("Failed to get current exe path");

        // Append the relative path to your asset
        path.push("assets");
        path.push("beep.wav");

        path.display().to_string()
    }

    // Helper that returns the full paths for the main and click files
    // It does not check if they exist
    fn get_file_paths(song_folder: &str, song_title: &str) -> (PathBuf, PathBuf) {
        let mut track_path = PathBuf::new();
        track_path.push(song_folder);
        track_path.push(format!("{}.wav", song_title));

        let mut click_path = PathBuf::new();
        click_path.push(song_folder);
        click_path.push(format!("{}_click.wav", song_title));

        (track_path, click_path)
    }

    fn decompress_files(song_folder: &str, song_title: &str) -> Result<(), Box<dyn Error>> {
        // if there's a 7z file with the same name, decompress it
        //let archive_path = PathBuf::from(format!("{}/{}/{}.7z", music_folder, song.folder, song.file_name));
        let mut archive_path = PathBuf::new();
        archive_path.push(song_folder);
        archive_path.push(format!("{}.7z", song_title));

        let mut output_folder = PathBuf::new();
        output_folder.push(song_folder);

        match sevenz_rust::decompress_file(&archive_path, output_folder) {
            Ok(_) => {
                info!("Decompressed file: {:?}", archive_path);
                Ok(())
            }
            Err(err) => {
                error!("Failed to decompress file: {:?}", err);
                Err(Box::new(err))
            }
        }
    }
}
