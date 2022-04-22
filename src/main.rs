extern crate env_logger;
extern crate librespot_audio;
extern crate librespot_core;
extern crate librespot_metadata;

#[macro_use]
extern crate log;
extern crate regex;
extern crate scoped_threadpool;
extern crate tokio;

use std::env;
use std::fs::File;
use std::process;
use regex::Regex;
use std::io::{self, BufRead, Read, Result};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;


use env_logger::{Builder, Env};

use librespot_audio::{AudioDecrypt, AudioFile};

use librespot_core::authentication::Credentials;
use librespot_core::config::SessionConfig;
use librespot_core::session::Session;
use librespot_core::spotify_id::SpotifyId;
use librespot_metadata::{Artist, Playlist, FileFormat, Metadata, Track, Album};


#[tokio::main]
async fn main() {
    Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<_> = env::args().collect();
    assert!(args.len() == 3 || args.len() == 4, "Usage: {} user password [helper_script] < tracks_file", args[0]);

    let session_config = SessionConfig::default();
    let credentials = Credentials::with_password(args[1].to_owned(), args[2].to_owned());

    let plist_uri = SpotifyId::from_uri(&args[3]).unwrap_or_else(|_| {
        eprintln!(
            "PLAYLIST should be a playlist URI such as: \
                \"spotify:playlist:37i9dQZF1DXec50AjHrNTq\""
        );
        process::exit(1);
    });
   

    info!("Connecting...");
    let session = Session::connect(session_config, credentials, None)
        .await
        .unwrap();
    info!("Connected!");

    let play_list = Playlist::get(&session, plist_uri).await.unwrap();
    info!("Playlist Uri {}", play_list.name);
    
    println!("{:?}", play_list);
    for track_id in play_list.tracks {
        let track = Track::get(&session, track_id).await.unwrap();
        println!("track: {} ", track.name);

        let file_id = track.files.get(&FileFormat::OGG_VORBIS_160).unwrap();
        println!("FileID: {}", file_id);
   
        let audio_key = session.audio_key().request(track.id, *file_id).await.unwrap();
        println!("Key: {:?}", audio_key);

        let mut encrypted_file = AudioFile::open(&session, *file_id, 160, true).await.unwrap();
        let mut buffer = Vec::new();
        

        let mut read_all: Result<usize> = Ok(0);
        let fetched = AtomicBool::new(false);
        read_all = encrypted_file.read_to_end(&mut buffer);
        fetched.store(true, Ordering::Release);

        while !fetched.load(Ordering::Acquire) {
            Some(Duration::from_millis(100));
        }

        read_all.expect("Cannot read file stream");


        let mut decrypted_buffer = Vec::new();
        AudioDecrypt::new(audio_key, &buffer[..]).read_to_end(&mut decrypted_buffer).expect("Cannot decrypt stream");

        let fname = format!("{}.ogg", track.name);
        std::fs::write(&fname, &decrypted_buffer[0xa7..]).expect("Cannot write decrypted track");
        info!("Filename: {}", fname);
    }
    
    let spotify_uri = Regex::new(r"spotify:track:([[:alnum:]]+)").unwrap();
    let spotify_url = Regex::new(r"open\.spotify\.com/track/([[:alnum:]]+)").unwrap();
}