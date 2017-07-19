use std::error::Error;

use futures::{Future, BoxFuture, Async, Poll};
use futures::future;
use rustfm_scrobble;

use metadata::Track;
use session::Session;
use util::SpotifyId;

pub struct Scrobbler {
    session: Session,
    scrobbler: rustfm_scrobble::Scrobbler,

    current_track_id: Option<SpotifyId>,

    auth: BoxFuture<(), rustfm_scrobble::ScrobblerError>
}

impl Scrobbler {

    pub fn new(session: Session) -> Scrobbler {
        let scrobbler = rustfm_scrobble::Scrobbler::new(String::new(), String::new());

        let mut ret = Scrobbler {
            session: session,
            scrobbler: scrobbler,
            current_track_id: None,
            auth: future::empty().boxed()
        };
        ret.start_auth();
        ret
    }

    pub fn start_auth(&mut self) {
        self.auth = self.auth();
    }

    pub fn auth(&mut self) -> BoxFuture<(), rustfm_scrobble::ScrobblerError> {
        println!("In auth future");
        match self.scrobbler.authenticate(String::new(), String::new()) {
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

            },
            Ok(Async::NotReady) => {

            },
            Err(err) => {
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
