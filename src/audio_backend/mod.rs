use std::io;

pub trait Open {
    fn open(Option<String>) -> Self;
}

pub trait Sink {
    fn start(&mut self) -> io::Result<()>;
    fn stop(&mut self) -> io::Result<()>;
    fn write(&mut self, data: &[i16]) -> io::Result<()>;
}

/*
 * Allow #[cfg] rules around elements of a list.
 * Workaround until stmt_expr_attributes is stable.
 *
 * This generates 2^n declarations of the list, with every combination possible
 */
macro_rules! declare_backends {
    (pub const $name:ident : $ty:ty = & [ $($tt:tt)* ];) => (
        _declare_backends!($name ; $ty ; []; []; []; $($tt)*);
    );
}

macro_rules! _declare_backends {
    ($name:ident ; $ty:ty ; [ $($yes:meta,)* ] ; [ $($no:meta,)* ] ; [ $($exprs:expr,)* ] ; #[cfg($m:meta)] $e:expr, $($rest:tt)* ) => (
        _declare_backends!($name ; $ty ; [ $m, $($yes,)* ] ; [ $($no,)* ] ; [ $($exprs,)* $e, ] ; $($rest)*);
        _declare_backends!($name ; $ty ; [ $($yes,)* ] ; [ $m, $($no,)* ] ; [ $($exprs,)* ] ; $($rest)*);
    );

    ($name:ident ; $ty:ty ; [ $($yes:meta,)* ] ; [ $($no:meta,)* ] ; [ $($exprs:expr,)* ] ; $e:expr, $($rest:tt)*) => (
        _declare_backends!($name ; $ty ; [ $($yes,)* ] ; [ $($no,)* ] ; [ $($exprs,)* $e, ] ; $($rest)*);
    );

    ($name:ident ; $ty:ty ; [ $($yes:meta,)* ] ; [ $($no:meta,)* ] ; [ $($exprs:expr,)* ] ; #[cfg($m:meta)] $e:expr) => (
        _declare_backends!($name ; $ty ; [ $m, $($yes,)* ] ; [ $($no,)* ] ; [ $($exprs,)* $e, ] ; );
        _declare_backends!($name ; $ty ; [ $($yes,)* ] ; [ $m, $($no,)* ] ; [ $($exprs,)* ] ; );
    );

    ($name:ident ; $ty:ty ; [ $($yes:meta,)* ] ; [ $($no:meta,)* ] ; [ $($exprs:expr,)* ] ; $e:expr ) => (
        _declare_backends!($name ; $ty ; [ $($yes,)* ] ; [ $($no,)* ] ; [ $($exprs,)* $e, ] ; );
    );

    ($name:ident ; $ty:ty ; [ $($yes:meta,)* ] ; [ $($no:meta,)* ] ; [ $($exprs:expr,)* ] ; ) => (
        #[cfg(all($($yes,)* not(any($($no),*))))]
        pub const $name : $ty = &[
            $($exprs,)*
        ];
    )
}

fn mk_sink<S: Sink + Open + 'static>(device: Option<String>) -> Box<Sink> {
    Box::new(S::open(device))
}

mod pipe;
use self::pipe::StdoutSink;

declare_backends! {
    pub const BACKENDS : &'static [
        (&'static str, fn(Option<String>) -> Box<Sink>)
    ] = &[
        ("pipe", mk_sink::<StdoutSink>),
    ];
}

pub fn find<T: AsRef<str>>(name: Option<T>) -> Option<fn(Option<String>) -> Box<Sink>> {
    if let Some(name) = name.as_ref().map(AsRef::as_ref) {
        BACKENDS.iter().find(|backend| name == backend.0).map(|backend| backend.1)
    } else {
        Some(BACKENDS.first().expect("No backends were enabled at build time").1)
    }
}
