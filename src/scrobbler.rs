use std::error::Error;

use futures::{Future, BoxFuture, Async, Poll};
use futures::future;
use rustfm_scrobble;

use metadata::Track;
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

    session: Option<Session>,
    current_track_id: Option<SpotifyId>,

    auth: BoxFuture<(), rustfm_scrobble::ScrobblerError>
}

impl Scrobbler {

    pub fn new(config: ScrobblerConfig) -> Scrobbler {
        info!("{:?}", config);
        let mut scrobbler = Scrobbler {
            session: None,
            scrobbler: rustfm_scrobble::Scrobbler::new(config.api_key.clone(), config.api_secret.clone()),
            current_track_id: None,
            auth: future::empty().boxed(),
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

}

impl Future for Scrobbler {
    type Item = Result<(), ()>;
    type Error = ();

    fn poll(&mut self) -> Poll<Result<(), ()>, ()> {
        match self.auth.poll() {
            Ok(Async::Ready(_)) => {
                info!("Authenticated with Last.fm")
            },
            Ok(Async::NotReady) => {
                return Ok(Async::NotReady)
            },
            Err(err) => {
                error!("Authentication error: {:?}", err);
                return Err(())
            }
        }

        /*if !self.authed {
            println!("Detected no auth");
            self.auth().and_then(|result| {
                println!("In and_then");
                self.authed = true;
                Ok(())
            });

            return Ok(Async::NotReady)
        } else {
            println!("Authed");
        }*/

        Ok(Async::NotReady)
    }

}
