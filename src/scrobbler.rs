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

    auth: BoxFuture<(), rustfm_scrobble::ScrobblerError>,
    now_playing: BoxFuture<(), ScrobbleError>,
    meta_fetch: BoxFuture<(String, String), ScrobbleError>
}
unsafe impl Send for Scrobbler {}
unsafe impl Sync for Scrobbler {}

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
            auth: future::empty().boxed(),
            now_playing: future::empty().boxed(),
            meta_fetch: future::empty().boxed(),
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

        match self.meta_fetch.poll() {
            Ok(Async::Ready((artist, track))) => {
                //info!("Metadata result: {:?}", meta);
                self.meta_fetch = future::empty().boxed();
                self.now_playing = self.send_now_playing(track, artist);
            },
            Ok(Async::NotReady) => {
                //return Ok(Async::NotReady)
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
                return Ok(Async::NotReady)
            },
            Err(err) => {
                error!("Now Playing error: {:?}", err);
                return Err(())
            }
        }

        Ok(Async::NotReady)
    }

}
