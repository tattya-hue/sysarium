//! シグナルハンドラではフラグだけを更新し，通常のループ終了で端末を復旧する．
#[cfg(unix)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct Shutdown {
    #[cfg(unix)]
    requested: Arc<AtomicBool>,
    #[cfg(unix)]
    registrations: Vec<signal_hook::SigId>,
}

impl Shutdown {
    pub fn install() -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            let mut guard = Self {
                requested: Arc::new(AtomicBool::new(false)),
                registrations: Vec::new(),
            };
            for signal in [
                signal_hook::consts::SIGTERM,
                signal_hook::consts::SIGHUP,
                signal_hook::consts::SIGINT,
            ] {
                guard.registrations.push(signal_hook::flag::register(
                    signal,
                    Arc::clone(&guard.requested),
                )?);
            }
            Ok(guard)
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    pub fn requested(&self) -> bool {
        #[cfg(unix)]
        {
            self.requested.load(Ordering::Relaxed)
        }
        #[cfg(not(unix))]
        {
            false
        }
    }
}

impl Drop for Shutdown {
    fn drop(&mut self) {
        #[cfg(unix)]
        for registration in self.registrations.drain(..) {
            signal_hook::low_level::unregister(registration);
        }
    }
}
