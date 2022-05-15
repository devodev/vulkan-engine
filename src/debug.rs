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
        let _x = $crate::debug::Timing::new("TIME!");
    };
    ($msg: expr) => {
        let _x = $crate::debug::Timing::new($msg);
    };
}
pub(crate) use TIME;
