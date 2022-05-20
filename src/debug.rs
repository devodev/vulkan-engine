use std::{
    borrow::Cow,
    time::{Duration, Instant},
};

use log::{debug, error, info, trace, warn};

// inspired from: https://gitlab.com/imp/easytiming-rs/-/blob/master/src/lib.rs
pub struct Timing<'a> {
    start: Instant,
    level: log::Level,
    msg: Cow<'a, str>,
}

impl<'a> Default for Timing<'a> {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            level: log::Level::Trace,
            msg: "TIME!".into(),
        }
    }
}

impl<'a> Timing<'a> {
    pub fn new<N>(msg: N) -> Self
    where
        N: Into<Cow<'a, str>>,
    {
        let mut t = Self::default();
        t.msg = msg.into();
        t
    }

    #[inline]
    fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self.start)
    }

    #[inline]
    fn log(&self) {
        let msg = format!("[{:?}] {}", self.elapsed(), self.msg);
        match self.level {
            log::Level::Error => error!("{}", msg),
            log::Level::Warn => warn!("{}", msg),
            log::Level::Info => info!("{}", msg),
            log::Level::Debug => debug!("{}", msg),
            log::Level::Trace => trace!("{}", msg),
        }
    }
}

impl<'a> Drop for Timing<'a> {
    fn drop(&mut self) {
        self.log()
    }
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! TIME {
    () => {
        let _x = $crate::debug::Timing::new();
    };
    ($msg: expr) => {
        let _x = $crate::debug::Timing::new($msg);
    };
}
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! TIME {
    () => {
        ()
    };
    ($msg: expr) => {
        ()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const MSG: &str = "timing";

    #[test]
    fn fromstr() {
        let t: Timing = Timing::new(MSG);
        assert_eq!(t.msg, MSG);
    }

    #[test]
    fn fromstring() {
        let t: Timing = Timing::new(String::from(MSG));
        assert_eq!(t.msg, MSG);
    }

    #[test]
    fn fromborrowed() {
        let t: Timing = Timing::new(Cow::Borrowed(MSG));
        assert_eq!(t.msg, MSG);
    }

    #[test]
    fn fromowned() {
        let t: Timing = Timing::new(Cow::Owned(String::from(MSG)));
        assert_eq!(t.msg, MSG);
    }
}
