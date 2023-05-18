use clap::{Arg, ArgMatches};

//mod player;
//use player::run_cli;

mod common;
use common::{PlayerArguments, get_file_paths};
//mod ui;
//use ui::run_ui;

mod lib;
use lib::Player;

use cpal::Device;
use cpal::traits::HostTrait;

use crate::lib::Song;


fn main() {

    let matches = clap::Command::new("eDrums Wav Player")
        .version("0.1")
        .arg(Arg::new("music_folder").long("music_folder").required(true).help("Where your music files are stored"))
        .arg(Arg::new("track").long("track").required(true).help("Position in the csv file to play"))
        .arg(Arg::new("track_volume").long("track_volume").required(false).default_value("100"))
        .arg(Arg::new("click_volume").long("click_volume").required(false).default_value("100"))
        .arg(Arg::new("track_device").long("track_device").required(false).default_value("1"))
        .arg(Arg::new("click_device").long("click_device").required(false).default_value("1"))
        .arg(Arg::new("ui").long("ui").required(false).default_value("1"))
        .get_matches();

    if let Err(err) = run(&matches) {
        println!("Error: {}", err);
        std::process::exit(1);
    }

}

/// Runs the program as determined by the main function
fn run(matches: &ArgMatches) -> Result<i32, String> {

    // Parse the arguments
    let music_folder = matches.get_one::<String>("music_folder").expect("No folder provided");
    
    let track_position = matches.get_one::<String>("track")
        .unwrap_or(&"1.0".to_string())
        .parse::<usize>()
        .unwrap_or(1);

   let track_volume = matches.get_one::<String>("track_volume")
        .unwrap_or(&"1.0".to_string())
        .parse::<f32>()
        .unwrap_or(100.0) / 100.0;
    let click_volume = matches.get_one::<String>("click_volume")
        .unwrap_or(&"1.0".to_string())
        .parse::<f32>()
        .unwrap_or(100.0) / 100.0;
    let track_device_position = matches.get_one::<String>("track_device")
        .unwrap_or(&"1".to_string())
        .parse::<usize>()
        .unwrap_or(1) - 1;
    let click_device_position = matches.get_one::<String>("click_device")
        .unwrap_or(&"1".to_string())
        .parse::<usize>()
        .unwrap_or(1) - 1;
    let ui = matches
        .get_one::<String>("ui")
        .map(|value| value == "1")
        .unwrap_or(false);

    // make sure the file exists. If it doesn't try to find the compressed version and decompress it.
    let (track_file, click_file) = get_file_paths(music_folder, track_position);

    let arguments = PlayerArguments {  
        music_folder: music_folder.to_string(),
        track_song: track_file,
        click_song: click_file,
        track_volume: track_volume,
        click_volume: click_volume,
        track_device_position: track_device_position,
        click_device_position: click_device_position,
    };

    if ui {
        match run_ui(arguments) {
            Ok(_) => {},
            Err(err) => return Err(format!("Could not start the ui. {}", err)),
        }
    } else {
        match run_cli(arguments) {
            Ok(_) => {},
            Err(err) => return Err(format!("Could not run the cli: {}", err)),
        }
    }

    Ok(0)

}

fn run_cli(arguments: PlayerArguments) -> Result<i32, String> {
    play_song(arguments)?;
    Ok(0)
}

fn run_ui(arguments: PlayerArguments) -> Result<i32, String> {
    println!("Running ui");
    Ok(0)
}


fn play_song(arguments: PlayerArguments) -> Result<i32, String> {
    let host = cpal::default_host();
    let available_devices = host.output_devices().unwrap().collect::<Vec<_>>();

    let track_device = &available_devices[arguments.track_device_position];
    let click_device = &available_devices[arguments.track_device_position];

    let track_play = Player::new(None, track_device).expect("Could not create track player");
    let click_play = Player::new(None, click_device).expect("Could not create click player");

    let track_volume = Some(arguments.track_volume);
    let click_volume = Some(arguments.click_volume);

    let track_song = Song::from_file(arguments.track_song, track_volume).expect("Could not create track song");
    let click_song = Song::from_file(arguments.click_song, click_volume).expect("Could not create click song");

    track_play.play_song_now(&track_song, None).expect("Could not play track song");
    click_play.play_song_now(&click_song, None).expect("Could not play click song");

    while track_play.has_current_song() && click_play.has_current_song() {
        std::thread::sleep(std::time::Duration::from_secs(1));

        // let (track_samples, track_position) = track_play.get_playback_position().expect("Could not get track playback position");
        // let (click_samples, click_position) = click_play.get_playback_position().expect("Could not get click playback position");

        // println!("Track: {}/{} Click: {}/{}", 
        //     track_position.as_secs(), 
        //     track_samples.as_secs(), 
        //     click_position.as_secs(), 
        //     click_samples.as_secs());
        
    }

    Ok(0)
}