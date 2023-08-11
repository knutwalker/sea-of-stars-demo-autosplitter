#![no_std]

use asr::{
    future::next_tick,
    game_engine::unity::il2cpp::{Class, Module, Version},
    timer::{self, TimerState},
    user_settings::{Settings, Title},
    watcher::Watcher,
    Address, Address64, Process,
};
use core::fmt::{self, Debug};

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
                    if matches!(timer::state(), TimerState::NotRunning | TimerState::Ended)
                        && !matches!(progress.splits, SplitProgression::NotRunning { .. })
                    {
                        progress = Progress::new();
                    }

                    while let Some(action) = progress.act(&data) {
                        log!("Decided on an action: {action:?}");
                        match action {
                            Action::ResetAndStart => {
                                log!("Starting new run");
                                if timer::state() == TimerState::Ended {
                                    timer::reset();
                                }
                                timer::start();
                            }
                            Action::Split(split) => settings.split(split),
                            Action::Pause => settings.pause(),
                            Action::Resume => settings.resume(),
                        }
                    }

                    next_tick().await;
                }
            })
            .await;
    }
}

#[derive(Settings)]
struct Settings {
    /// Additional Splits
    _splits: Title,
    /// Split when descending the mountain
    mountain: bool,
    /// Split when leaving town
    town: bool,
    /// Split when defeating the special mob in the blue room
    mob: bool,
    /// Split when leveled up
    level_up: bool,
    /// Split when starting the boss fight
    dungeon: bool,
    /// Miscellaneous
    _misc: Title,
    /// Stop game timer during loads
    stop_when_loading: bool,
}

impl Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("mountain", &self.mountain)
            .field("town", &self.town)
            .field("mob", &self.mob)
            .field("level_up", &self.level_up)
            .field("dungeon", &self.dungeon)
            .finish()
    }
}

impl Settings {
    fn pause(&self) {
        if self.stop_when_loading {
            log!("Pause game time");
            timer::pause_game_time();
        }
    }

    fn resume(&self) {
        if self.stop_when_loading {
            log!("Resume game time");
            timer::resume_game_time();
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

#[derive(Debug)]
enum Split {
    Mountain,
    Town,
    Mob,
    LevelUp,
    Dungeon,
    Boss,
}

#[derive(Debug)]
enum Action {
    ResetAndStart,
    Split(Split),
    Pause,
    Resume,
}

struct Progress {
    loading: Watcher<bool>,
    splits: SplitProgression,
    next: Option<Action>,
}

impl Progress {
    fn new() -> Self {
        Self {
            loading: Watcher::new(),
            splits: SplitProgression::new(),
            next: None,
        }
    }

    fn act(&mut self, data: &Data<'_>) -> Option<Action> {
        if let Some(next) = self.next.take() {
            return Some(next);
        }

        match self.loading.update(data.is_loading()) {
            Some(l) if l.changed_to(&false) => Some(Action::Resume),
            Some(l) if l.changed_to(&true) => {
                self.next = self.splits.act(true, data);
                Some(Action::Pause)
            }
            _ => self.splits.act(false, &data),
        }
    }
}

enum SplitProgression {
    NotRunning { play_time: Watcher<u64> },
    Started { level_loads: usize },
    InDungeon,
    AgainstMob,
    DungeonAgain { party_level: Watcher<u32> },
    Leveled,
    EncounteredFinalBoss { enemy: Address64, hp: Watcher<u32> },
}

impl SplitProgression {
    fn new() -> Self {
        let mut play_time = Watcher::new();
        play_time.update_infallible(u64::MAX);
        Self::NotRunning { play_time }
    }

    fn act(&mut self, loading: bool, data: &Data<'_>) -> Option<Action> {
        match self {
            Self::NotRunning { play_time } => {
                let play_time = play_time.update(data.play_time());
                if play_time.is_some_and(|pt| pt.changed_to(&0)) {
                    *self = Self::Started { level_loads: 0 };
                    return Some(Action::ResetAndStart);
                }
            }
            Self::Started { level_loads } => {
                if loading {
                    *level_loads += 1;
                    match *level_loads {
                        2 => return Some(Action::Split(Split::Mountain)),
                        3 => return Some(Action::Split(Split::Town)),
                        4 => {
                            *self = Self::InDungeon;
                        }
                        _ => {}
                    }
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
                let (enemy, encounter_hp) = data.first_enemy_start_hp().unwrap_or_default();

                if encounter_hp == 700 {
                    let mut hp = Watcher::new();
                    hp.update_infallible(700);

                    *self = Self::EncounteredFinalBoss { enemy, hp };
                    return Some(Action::Split(Split::Dungeon));
                }
            }
            Self::EncounteredFinalBoss { hp, enemy } => {
                let hp = hp.update(data.current_hp(*enemy));
                if hp.is_some_and(|hp| hp.changed_to(&0)) {
                    *self = Self::new();
                    return Some(Action::Split(Split::Boss));
                }
            }
        };

        None
    }
}

#[derive(Class)]
struct ProgressionManager {
    #[rename = "playTime"]
    play_time: f64,
}

#[derive(Class)]
struct LevelManager {
    #[rename = "loadingLevel"]
    is_loading: bool,
}

#[derive(Class)]
struct CharacterStatsManager {
    #[rename = "partyProgressData"]
    party_progress: Address64,
}

#[derive(Class)]
struct PartyData {
    #[rename = "currentLevel"]
    current_level: u32,
}

#[derive(Class)]
struct CombatManager {
    #[rename = "currentEncounter"]
    encounter: Address64,
}

#[derive(Class)]
struct Encounter {
    #[rename = "encounterDone"]
    done: bool,
    #[rename = "enemyTargets"]
    enemy_targets: Address64,
}

#[derive(Debug, Class)]
struct EnemyCombatTarget {
    #[rename = "currentHP"]
    current_hp: u32,
}

#[derive(Class)]
struct CombatTarget {
    owner: Address64,
}

#[derive(Class)]
struct EnemyCombatActor {
    #[rename = "enemyData"]
    data: Address64,
}

#[derive(Class)]
struct CharacterData {
    hp: u32,
}

struct Data<'a> {
    process: &'a Process,
    progression: Singleton<ProgressionManagerBinding>,
    level: Singleton<LevelManagerBinding>,
    char_stats: Singleton<CharacterStatsManagerBinding>,
    party_data: PartyDataBinding,
    combat: Singleton<CombatManagerBinding>,
    encounter: EncounterBinding,
    enemy_target: EnemyCombatTargetBinding,
    combat_target: CombatTargetBinding,
    enemy_actor: EnemyCombatActorBinding,
    char_data: CharacterDataBinding,
}

impl Data<'_> {
    fn play_time(&self) -> Option<u64> {
        Some(self.progression.read(self.process)?.play_time as _)
    }

    fn is_loading(&self) -> Option<bool> {
        Some(self.level.read(self.process)?.is_loading)
    }

    fn party_level(&self) -> Option<u32> {
        let stats = self.char_stats.read(self.process)?;
        let progress = self
            .party_data
            .read(self.process, stats.party_progress.into())
            .ok()?;
        Some(progress.current_level)
    }

    fn encounter_size(&self) -> Option<u32> {
        const LIST_SIZE: u64 = 0x18;

        let current_encounter = self.current_encounter()?;
        self.process
            .read(current_encounter.enemy_targets + LIST_SIZE)
            .ok()
    }

    fn encounter_done(&self) -> Option<bool> {
        let current_encounter = self.current_encounter()?;
        Some(current_encounter.done)
    }

    fn first_enemy_start_hp(&self) -> Option<(Address64, u32)> {
        let first_enemy = self.first_enemy()?;

        let combat_target = self
            .combat_target
            .read(self.process, first_enemy.into())
            .ok()?;

        let combat_actor = self
            .enemy_actor
            .read(self.process, combat_target.owner.into())
            .ok()?;

        let char_data = self
            .char_data
            .read(self.process, combat_actor.data.into())
            .ok()?;

        Some((first_enemy, char_data.hp))
    }

    fn current_hp(&self, enemy: Address64) -> Option<u32> {
        let enemy_target = self.enemy_target.read(self.process, enemy.into()).ok()?;
        Some(enemy_target.current_hp)
    }

    fn current_encounter(&self) -> Option<Encounter> {
        let combat = self.combat.read(self.process)?;
        self.encounter
            .read(self.process, combat.encounter.into())
            .ok()
    }

    fn first_enemy(&self) -> Option<Address64> {
        const SKIP_OBEJCT_HEADER: u64 = 0x10;
        const SKIP_ARRAY_HEADER: u64 = 0x20;

        let current_encounter = self.current_encounter()?;

        let first_enemy = self
            .process
            .read_pointer_path64::<Address64>(
                current_encounter.enemy_targets,
                &[SKIP_OBEJCT_HEADER, SKIP_ARRAY_HEADER],
            )
            .ok()?;

        Some(first_enemy)
    }
}

impl<'a> Data<'a> {
    async fn new(process: &'a Process) -> Data<'a> {
        let module = Module::wait_attach(process, Version::V2020).await;
        let image = module.wait_get_default_image(process).await;
        log!("Attached to the game");

        macro_rules! bind {
            ($cls:ty) => {{
                let binding = <$cls>::bind(process, &module, &image).await;
                log!(concat!("Created binding for class ", stringify!($cls)));
                binding
            }};
            (singleton $cls:ty) => {{
                let binding = <$cls>::bind(process, &module, &image).await;
                let address = binding
                    .class()
                    .wait_get_parent(process, &module)
                    .await
                    .wait_get_static_instance(process, &module, "instance")
                    .await;

                log!(concat!("found ", stringify!($cls), " at {}"), address);

                Singleton { binding, address }
            }};
        }

        Self {
            process,
            progression: bind!(singleton ProgressionManager),
            level: bind!(singleton LevelManager),
            char_stats: bind!(singleton CharacterStatsManager),
            party_data: bind!(PartyData),
            combat: bind!(singleton CombatManager),
            encounter: bind!(Encounter),
            enemy_target: bind!(EnemyCombatTarget),
            combat_target: bind!(CombatTarget),
            char_data: bind!(CharacterData),
            enemy_actor: bind!(EnemyCombatActor),
        }
    }
}

struct Singleton<T> {
    binding: T,
    address: Address,
}

macro_rules! impl_binding {
    ($($cls:ty),+ $(,)?) => {
        $(::paste::paste! {
            impl Singleton<[<$cls Binding>]> {
                fn read(&self, process: &Process) -> Option<$cls> {
                    self.binding.read(process, self.address).ok()
                }
            }
        })+
    };
}

impl_binding!(
    ProgressionManager,
    LevelManager,
    CharacterStatsManager,
    CombatManager,
);
