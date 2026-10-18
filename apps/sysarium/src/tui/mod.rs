mod app;
mod renderer;
use crate::shutdown;
mod terminal;

use app::App;
use crossterm::event;
use std::{
    io,
    time::{Duration, Instant},
};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const FRAME_INTERVAL: Duration = Duration::from_millis(100);

pub fn run() -> io::Result<()> {
    let shutdown = shutdown::Shutdown::install()?;
    terminal::with_terminal(|terminal| {
        let (first, mut sample_stamp) = crate::sample(1, None)?;
        let mut app = App::new(first);
        let started = Instant::now();
        let mut next_sample = sample_stamp + SAMPLE_INTERVAL;
        let mut next_frame = started;
        loop {
            if shutdown.requested() {
                return Ok(());
            }
            if Instant::now() >= next_sample {
                let sample_id = app
                    .analysis
                    .source_sample_id
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("sample_id overflow"))?;
                let (observation, stamp) = crate::sample(sample_id, Some(sample_stamp))?;
                sample_stamp = stamp;
                app.observe(observation);
                // 遅延時に取り戻しサンプリングを連発しない．
                next_sample = Instant::now() + SAMPLE_INTERVAL;
            }
            if Instant::now() >= next_frame {
                let size = terminal.size()?;
                app.resize(size.width, size.height);
                terminal.draw(|frame| renderer::draw(frame, &app, started.elapsed()))?;
                next_frame = Instant::now() + FRAME_INTERVAL;
            }
            let timeout = next_sample
                .min(next_frame)
                .saturating_duration_since(Instant::now());
            if event::poll(timeout)? && app.handle_event(event::read()?) {
                return Ok(());
            }
        }
    })
}

#[cfg(test)]
mod tests;
