#![no_std]

use crate::{
    data::Data,
    progress::{Action, Progress},
    settings::Settings,
};
use asr::{
    future::next_tick,
    timer::{self, TimerState},
    Process,
};
use progress::Split;

#[cfg(debug_assertions)]
macro_rules! log {
    ($($arg:tt)*) => {{
        let mut buf = ::arrayvec::ArrayString::<1024>::new();
        let _ = ::core::fmt::Write::write_fmt(
            &mut buf,
            ::core::format_args!($($arg)*),
        );
        ::asr::print_message(&buf);
    }};
}

#[cfg(not(debug_assertions))]
macro_rules! log {
    ($($arg:tt)*) => {};
}

mod data;
mod progress;
mod settings;

asr::async_main!(stable);
asr::panic_handler!();

async fn main() {
    asr::set_tick_rate(60.0);
    let settings = Settings::register();
    log!("Loaded settings: {settings:?}");

    loop {
        let process = Process::wait_attach("SeaOfStars.exe").await;
        process
            .until_closes(async {
                let data = Data::new(&process).await;
                let mut progress = Progress::new();

                loop {
                    if matches!(timer::state(), TimerState::NotRunning | TimerState::Ended) {
                        progress.reset();
                    }

                    while let Some(action) = progress.act(&data) {
                        log!("Possible action: {action:?}");
                        if let Some(action) = settings.filter(action) {
                            log!("Decided on an action: {action:?}");
                            act(action);
                        }
                    }

                    next_tick().await;
                }
            })
            .await;
    }
}

fn act(action: Action) {
    match action {
        Action::ResetAndStart => {
            log!("Starting new run");
            if timer::state() == TimerState::Ended {
                timer::reset();
            }
            timer::start();
        }
        Action::Split(split) => match split {
            Split::Mountain => {
                log!("Climbed down the mountain");
                timer::split();
            }
            Split::Town => {
                log!("Left town");
                timer::split();
            }
            Split::Mob => {
                log!("Bested a mob of fiends");
                timer::split();
            }
            Split::LevelUp => {
                log!("Party leveled up");
                timer::split();
            }
            Split::Dungeon => {
                log!("Encountering final boss");
                timer::split();
            }
            Split::Boss => {
                log!("Run is finished");
                timer::split();
            }
        },
        Action::Pause => {
            log!("Pause game time");
            timer::pause_game_time();
        }
        Action::Resume => {
            log!("Resume game time");
            timer::resume_game_time();
        }
    }
}
