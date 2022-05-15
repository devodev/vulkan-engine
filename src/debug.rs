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
    #[inline]
    fn drop(&mut self) {
        let elapsed = Instant::now().duration_since(self.start);
        debug!("{}", format!("[{:?}] {}", elapsed, self.msg))
    }
}

macro_rules! TIME {
    () => {
        let _x = Timing::new("TIME!");
    };
    ($base: expr) => {
        let _x = Timing::new($base);
    };
    ($base: expr, $($args:tt)*) => {
        let _msg = format!($base, $($args)*);
        let _x = Timing::new(&_msg);
    };
}
pub(crate) use TIME;
