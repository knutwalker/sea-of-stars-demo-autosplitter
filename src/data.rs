use asr::{
    game_engine::unity::il2cpp::{Class, Module, Version},
    Address, Address64, Process,
};

pub struct Data<'a> {
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
    pub fn play_time(&self) -> Option<u64> {
        Some(self.progression.read(self.process)?.play_time as _)
    }

    pub fn is_loading(&self) -> Option<bool> {
        Some(self.level.read(self.process)?.is_loading)
    }

    pub fn party_level(&self) -> Option<u32> {
        let stats = self.char_stats.read(self.process)?;
        let progress = self
            .party_data
            .read(self.process, stats.party_progress.into())
            .ok()?;
        Some(progress.current_level)
    }

    pub fn encounter_size(&self) -> Option<u32> {
        const LIST_SIZE: u64 = 0x18;

        let current_encounter = self.current_encounter()?;
        self.process
            .read(current_encounter.enemy_targets + LIST_SIZE)
            .ok()
    }

    pub fn encounter_done(&self) -> Option<bool> {
        let current_encounter = self.current_encounter()?;
        Some(current_encounter.done)
    }

    pub fn first_enemy_start_hp(&self) -> Option<(Address64, u32)> {
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

    pub fn current_hp(&self, enemy: Address64) -> Option<u32> {
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

impl<'a> Data<'a> {
    pub async fn new(process: &'a Process) -> Data<'a> {
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
