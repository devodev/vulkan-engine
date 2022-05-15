use std::time::Instant;

use log::debug;

pub struct Timing {
    start: Instant,
    msg: &'static str,
}

impl Timing {
    pub fn new(msg: &'static str) -> Self {
        Self {
            start: Instant::now(),
            msg,
        }
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        let elapsed = Instant::now().duration_since(self.start);
        debug!("{}", format!("[{:?}] {}", elapsed, self.msg))
    }
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! TIME {
    ($msg: expr) => {
        let _x = $crate::debug::Timing::new($msg);
    };
}
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! TIME {
    ($msg: expr) => {
        ()
    };
}
