use std::time::{Duration, Instant};

use futures::{Future, BoxFuture, Async, Poll};
use futures::future;
use rustfm_scrobble;

use metadata::{Track, Artist};
use session::Session;
use util::SpotifyId;

#[derive(Clone, Debug)]
pub struct ScrobblerConfig {
    pub api_key: String,
    pub api_secret: String,
    pub username: String,
    pub password: String
}

pub struct Scrobbler {
    config: ScrobblerConfig,
    scrobbler: rustfm_scrobble::Scrobbler,

    session: Session,
    current_track_id: Option<SpotifyId>,
    current_track_start: Option<Instant>,
    current_track_meta: Option<(String, String)>,
    current_track_scrobbled: bool,

    auth: BoxFuture<(), rustfm_scrobble::ScrobblerError>,
    now_playing: BoxFuture<(), ScrobbleError>,
    meta_fetch: BoxFuture<(String, String), ScrobbleError>,
    scrobble_future: Option<BoxFuture<(), ScrobbleError>>
}

#[derive(Debug)]
pub struct ScrobbleError {
    msg: String
}

impl ScrobbleError {

    pub fn new(msg: String) -> ScrobbleError {
        ScrobbleError {
            msg: msg
        }
    }

}

impl Scrobbler {

    pub fn new(config: ScrobblerConfig, session: Session) -> Scrobbler {
        let mut scrobbler = Scrobbler {
            session: session,
            scrobbler: rustfm_scrobble::Scrobbler::new(config.api_key.clone(), config.api_secret.clone()),
            current_track_id: None,
            current_track_start: None,
            current_track_meta: None,
            current_track_scrobbled: false,
            auth: future::empty().boxed(),
            now_playing: future::empty().boxed(),
            meta_fetch: future::empty().boxed(),
            scrobble_future: None,
            config: config
        };

        scrobbler.start_auth();
        scrobbler
    }

    pub fn start_auth(&mut self) {
        self.auth = self.auth();
    }

    pub fn auth(&mut self) -> BoxFuture<(), rustfm_scrobble::ScrobblerError> {
        match self.scrobbler.authenticate(self.config.username.clone(), self.config.password.clone()) {
            Ok(_) => future::ok(()),
            Err(err) => future::err(err)
        }.boxed()
    }

    pub fn update_current_track(&mut self, track_id: SpotifyId) {
        // TODO: Doesn't understand when a track is played on repeat
        let mut new_track = false;

        match self.current_track_id {
            None => { 
                new_track = true;
            },
            Some(id) => {
                if id != track_id {
                    new_track = true;
                }
            }
        }

        if !new_track {
            return
        }

        self.current_track_id = Some(track_id.clone());
        self.current_track_start = Some(Instant::now());
        self.current_track_meta = None;
        self.current_track_scrobbled = false;
        self.meta_fetch = self.get_track_meta(track_id.clone());
    }

    pub fn get_track_meta(&mut self, track_id: SpotifyId) -> BoxFuture<(String, String), ScrobbleError> {
        let metadata = self.session.metadata().clone();

        metadata.get::<Track>(track_id).and_then(move |track| {
            let track_name = track.name;
            let artist = *track.artists.first().expect("No artists");
            metadata.get::<Artist>(artist).map(|artist| (track_name, artist.name.clone()))
        }).map_err(move |err| {
            ScrobbleError::new(format!("{:?}", err).to_owned())
        }).and_then(move |(track, artist)| {
            future::ok((track.clone(), artist.clone()))
        }).boxed()
    }

    pub fn send_now_playing(&self, artist: String, track: String) -> BoxFuture<(), ScrobbleError> {
        info!("Now-playing scrobble: {} - {}", artist, track);

        match self.scrobbler.now_playing(track, artist) {
            Ok(_) => future::ok(()),
            Err(err) => future::err(ScrobbleError::new(format!("{:?}", err)))
        }.boxed()
    }

    pub fn send_scrobble(&self) -> BoxFuture<(), ScrobbleError> {
        match self.current_track_meta {
            Some(ref meta) => {
                let (artist, track) = meta.clone();
                info!("Scrobbling: {} - {}", artist, track);

                match self.scrobbler.scrobble(track, artist) {
                    Ok(_) => future::ok(()),
                    Err(err) => future::err(ScrobbleError::new(format!("{:?}", err)))
                }.boxed()
            },
            None => future::err(ScrobbleError::new("No track metadata available".to_string())).boxed()
        }
    }

    fn can_scrobble_track(&self) -> bool {
        if self.current_track_scrobbled {
            return false
        }

        match self.scrobble_future {
            Some(_) => {
                return false
            },
            None => {}
        }

        match self.current_track_start {
            Some(start_time) => {
                let play_time = start_time.elapsed();
                
                if play_time > Duration::new(20, 0) {
                    return true
                }

                false
            },
            _ => false
        }
    }

}

impl Future for Scrobbler {
    type Item = Result<(), ()>;
    type Error = ();

    fn poll(&mut self) -> Poll<Result<(), ()>, ()> {

        match self.auth.poll() {
            Ok(Async::Ready(_)) => {
                info!("Authenticated with Last.fm");
                self.auth = future::empty().boxed();
            },
            Ok(Async::NotReady) => {
                
            },
            Err(err) => {
                error!("Authentication error: {:?}", err);
                return Err(())
            }
        }

        if self.can_scrobble_track() {
            self.scrobble_future = Some(self.send_scrobble());
        }

        let mut track_scrobbled = false;
        match self.scrobble_future {
            Some(ref mut scrobble_future) => {
                match scrobble_future.poll() {
                    Ok(Async::Ready(_)) => {
                        track_scrobbled = true;
                    },
                    Ok(Async::NotReady) => {

                    },
                    Err(err) => {
                        error!("Scrobbling error: {:?}", err);
                        return Err(())
                    }
                }
            },
            None => ()
        }

        if track_scrobbled {
            self.scrobble_future = None;
            self.current_track_scrobbled = true;
        }

        match self.meta_fetch.poll() {
            Ok(Async::Ready((track, artist))) => {
                self.meta_fetch = future::empty().boxed();
                self.now_playing = self.send_now_playing(artist.clone(), track.clone());
                self.current_track_meta = Some((artist.clone(), track.clone()));
            },
            Ok(Async::NotReady) => {
                
            },
            Err(err) => {
                error!("Metadata fetch error: {:?}", err);
                return Err(())
            }
        }

        match self.now_playing.poll() {
            Ok(Async::Ready(_)) => {
                self.now_playing = future::empty().boxed();
            },
            Ok(Async::NotReady) => {
                
            },
            Err(err) => {
                error!("Now Playing error: {:?}", err);
                return Err(())
            }
        }



        Ok(Async::NotReady)
    }

}
