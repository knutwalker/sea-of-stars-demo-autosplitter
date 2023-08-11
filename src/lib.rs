#![no_std]

use asr::{
    future::next_tick,
    game_engine::unity::il2cpp::{Image, Module, Version},
    timer::{self, TimerState},
    user_settings,
    watcher::Watcher,
    Address, Address64, Process,
};

asr::async_main!(stable);
asr::panic_handler!();

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

async fn main() {
    let settings = Settings::register();
    log!("Loaded settings: {settings:?}");

    loop {
        let process = Process::wait_attach("SeaOfStars.exe").await;
        process
            .until_closes(async {
                let data = Data::new(&process).await;
                let mut progress = Progress::new();

                loop {
                    if matches!(timer::state(), TimerState::NotRunning | TimerState::Ended)
                        && !matches!(progress, Progress::NotRunning { .. })
                    {
                        progress = Progress::new();
                    }

                    let action = progress.act(&data);
                    match action {
                        Some(Action::ResetAndStart) => {
                            log!("Starting new run");
                            if timer::state() == TimerState::Ended {
                                timer::reset();
                            }
                            timer::start();
                        }
                        Some(Action::Pause(split)) => {
                            if settings.stop_when_loading {
                                log!("Pausing game time");
                                timer::pause_game_time();
                            }
                            if let Some(split) = split {
                                settings.split(split);
                            }
                        }
                        Some(Action::Resume) => {
                            if settings.stop_when_loading {
                                log!("Resuming game time");
                                timer::resume_game_time();
                            }
                        }
                        Some(Action::Split(split)) => settings.split(split),
                        None => {}
                    }

                    next_tick().await;
                }
            })
            .await;
    }
}

#[derive(Debug)]
struct Settings {
    mountain: bool,
    town: bool,
    mob: bool,
    level_up: bool,
    dungeon: bool,
    stop_when_loading: bool,
}

impl Settings {
    fn register() -> Self {
        Self {
            mountain: {
                user_settings::add_title("__sosd_splits", "Additional Splits", 0);
                user_settings::add_bool("mountain", "Split when descending the mountain", false)
            },
            town: user_settings::add_bool("town", "Split when leaving town", false),
            mob: user_settings::add_bool(
                "mob",
                "Split when defeating the special mob in the blue room",
                false,
            ),
            level_up: user_settings::add_bool("level_up", "Split when leveled up", false),
            dungeon: user_settings::add_bool(
                "dungeon",
                "Split when starting the boss fight",
                false,
            ),
            stop_when_loading: {
                user_settings::add_title("__sosd_misc", "Miscellaneous", 0);
                user_settings::add_bool("stop_when_loading", "Stop game timer during loads", false)
            },
        }
    }

    fn split(&self, split: Split) {
        match split {
            Split::Mountain => {
                if self.mountain {
                    log!("Climbed down the mountain");
                    timer::split();
                }
            }
            Split::Town => {
                if self.town {
                    log!("Left town");
                    timer::split();
                }
            }
            Split::Mob => {
                if self.mob {
                    log!("Bested a mob of fiends");
                    timer::split();
                }
            }
            Split::LevelUp => {
                if self.level_up {
                    log!("Party leveled up");
                    timer::split();
                }
            }
            Split::Dungeon => {
                if self.dungeon {
                    log!("Encountering final boss");
                    timer::split();
                }
            }
            Split::Boss => {
                log!("Run is finished");
                timer::split();
            }
        };
    }
}

enum Split {
    Mountain,
    Town,
    Mob,
    LevelUp,
    Dungeon,
    Boss,
}

enum Action {
    ResetAndStart,
    Split(Split),
    Pause(Option<Split>),
    Resume,
}

enum Progress {
    NotRunning {
        play_time: Watcher<u64>,
    },
    Started {
        loading: Watcher<bool>,
        level_loads: usize,
    },
    InDungeon,
    AgainstMob,
    DungeonAgain {
        party_level: Watcher<u32>,
    },
    Leveled,
    EncounteredFinalBoss {
        boss: Address64,
        hp: Watcher<u32>,
    },
}

impl Progress {
    fn new() -> Self {
        Self::NotRunning {
            play_time: Watcher::new(),
        }
    }

    fn act(&mut self, data: &Data<'_>) -> Option<Action> {
        match self {
            Self::NotRunning { play_time } => {
                let play_time = play_time.update(data.play_time());
                if play_time.is_some_and(|pt| pt.changed_to(&0)) {
                    *self = Self::Started {
                        loading: Watcher::new(),
                        level_loads: 0,
                    };
                    return Some(Action::ResetAndStart);
                }
            }
            Self::Started {
                loading,
                level_loads,
            } => {
                let loads = loading.update(data.is_loading());
                match loads {
                    Some(l) if l.changed_to(&true) => {
                        *level_loads += 1;
                        let split = match *level_loads {
                            2 => Some(Split::Mountain),
                            3 => Some(Split::Town),
                            _ => None,
                        };
                        return Some(Action::Pause(split));
                    }
                    Some(l) if l.changed_to(&false) => {
                        if *level_loads == 4 {
                            *self = Self::InDungeon;
                        }
                        return Some(Action::Resume);
                    }
                    _ => {}
                }
            }
            Self::InDungeon => {
                let encounter_size = data.encounter_size();
                if encounter_size.is_some_and(|es| es == 4) {
                    *self = Self::AgainstMob;
                }
            }
            Self::AgainstMob => {
                let encounter_done = data.encounter_done();
                if encounter_done.is_some_and(|d| d) {
                    *self = Self::DungeonAgain {
                        party_level: Watcher::new(),
                    };
                    return Some(Action::Split(Split::Mob));
                }
            }
            Self::DungeonAgain { party_level } => {
                let level = party_level.update(data.party_level());
                if level.is_some_and(|l| l.changed_to(&4)) {
                    *self = Self::Leveled;
                    return Some(Action::Split(Split::LevelUp));
                }
            }
            Self::Leveled => {
                let encounter_hp = data.first_enemy_start_hp().unwrap_or_default();

                if encounter_hp == 700 {
                    let boss = data.first_enemy().unwrap_or(Address64::NULL);
                    let mut hp = Watcher::new();
                    hp.update_infallible(700);
                    *self = Self::EncounteredFinalBoss { boss, hp };
                    return Some(Action::Split(Split::Dungeon));
                }
            }
            Self::EncounteredFinalBoss { boss, hp } => {
                let hp = hp.update(data.first_enemy_current_hp(*boss));
                if hp.is_some_and(|hp| hp.changed_to(&0)) {
                    *self = Self::new();
                    return Some(Action::Split(Split::Boss));
                }
            }
        };

        None
    }
}

struct Data<'a> {
    process: &'a Process,
    char_stats: Address,
    combat: Address,
    level: Address,
    progression: Address,
}

impl Data<'_> {
    const SKIP_OBEJCT_HEADER: u64 = 0x10;
    const SKIP_ARRAY_HEADER: u64 = 0x20;
    const LIST_SIZE: u64 = 0x18;

    const CURRENT_ENCOUNTER: u64 = 0xF0;
    const CURRENT_HP: u64 = 0x6C;
    const CURRENT_LEVEL: u64 = 0x18;
    const ENCOUNTER_DONE: u64 = 0x110;
    const ENEMY_ACTORS: u64 = 0x118;
    const ENEMY_DATA: u64 = 0x100;
    const ENEMY_TARGETS: u64 = 0x130;
    const IS_LOADING: u64 = 0x70;
    const MAX_HP: u64 = 0x20;
    const PARTY_PROGRESS: u64 = 0x68;
    const PLAY_TIME: u64 = 0x28;

    fn play_time(&self) -> Option<u64> {
        self.process.read(self.progression + Self::PLAY_TIME).ok()
    }

    fn is_loading(&self) -> Option<bool> {
        self.process.read(self.level + Self::IS_LOADING).ok()
    }

    fn encounter_size(&self) -> Option<u32> {
        self.process
            .read_pointer_path64(
                self.combat,
                &[
                    Self::CURRENT_ENCOUNTER,
                    Self::ENEMY_TARGETS,
                    Self::LIST_SIZE,
                ],
            )
            .ok()
    }

    fn encounter_done(&self) -> Option<bool> {
        self.process
            .read_pointer_path64(
                self.combat,
                &[Self::CURRENT_ENCOUNTER, Self::ENCOUNTER_DONE],
            )
            .ok()
    }

    fn party_level(&self) -> Option<u32> {
        self.process
            .read_pointer_path64(
                self.char_stats,
                &[Self::PARTY_PROGRESS, Self::CURRENT_LEVEL],
            )
            .ok()
    }

    fn first_enemy_start_hp(&self) -> Option<u32> {
        self.process
            .read_pointer_path64::<u32>(
                self.combat,
                &[
                    Self::CURRENT_ENCOUNTER,
                    Self::ENEMY_ACTORS,
                    Self::SKIP_OBEJCT_HEADER,
                    Self::SKIP_ARRAY_HEADER,
                    Self::ENEMY_DATA,
                    Self::MAX_HP,
                ],
            )
            .ok()
    }

    fn first_enemy(&self) -> Option<Address64> {
        self.process
            .read_pointer_path64(
                self.combat,
                &[
                    Self::CURRENT_ENCOUNTER,
                    Self::ENEMY_TARGETS,
                    Self::SKIP_OBEJCT_HEADER,
                    Self::SKIP_ARRAY_HEADER,
                ],
            )
            .ok()
    }

    fn first_enemy_current_hp(&self, encounter: Address64) -> Option<u32> {
        self.process.read(encounter + Self::CURRENT_HP).ok()
    }
}

impl<'a> Data<'a> {
    async fn new(process: &'a Process) -> Data<'a> {
        let module = Module::wait_attach(process, Version::V2020).await;
        let image = module.wait_get_default_image(process).await;

        let char_stats = Self::manager(&image, process, &module, "CharacterStatsManager").await;
        let combat = Self::manager(&image, process, &module, "CombatManager").await;
        let progression = Self::manager(&image, process, &module, "ProgressionManager").await;
        let level = Self::manager(&image, process, &module, "LevelManager").await;

        Self {
            process,
            char_stats,
            combat,
            level,
            progression,
        }
    }

    async fn manager(image: &Image, process: &Process, module: &Module, name: &str) -> Address {
        let instance = image
            .wait_get_class(process, module, name)
            .await
            .wait_get_parent(process, module)
            .await
            .wait_get_static_instance(process, module, "instance")
            .await;

        log!("found {name} at {instance}");

        instance
    }
}
