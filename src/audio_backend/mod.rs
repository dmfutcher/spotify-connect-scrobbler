use std::io;

pub trait Open {
    fn open(Option<String>) -> Self;
}

pub trait Sink {
    fn start(&mut self) -> io::Result<()>;
    fn stop(&mut self) -> io::Result<()>;
    fn write(&mut self, data: &[i16]) -> io::Result<()>;
}

fn mk_sink<S: Sink + Open + 'static>(device: Option<String>) -> Box<Sink> {
    Box::new(S::open(device))
}

mod nil;
use self::nil::NilSink;

pub const BACKENDS : &'static [
    (&'static str, fn(Option<String>) -> Box<Sink>)
] = &[
    ("nil", mk_sink::<NilSink>),
];

pub fn find(name: Option<String>) -> Option<fn(Option<String>) -> Box<Sink>> {
    if let Some(name) = name {
        BACKENDS.iter().find(|backend| name == backend.0).map(|backend| backend.1)
    } else {
        Some(BACKENDS.first().expect("No backends were enabled at build time").1)
    }
}
