use asr::{watcher::Watcher, Address64};

use crate::data::Data;

#[derive(Debug)]
pub enum Split {
    Mountain,
    Town,
    Mob,
    LevelUp,
    Dungeon,
    Boss,
}

#[derive(Debug)]
pub enum Action {
    ResetAndStart,
    Split(Split),
    Pause,
    Resume,
}

pub struct Progress {
    loading: Watcher<bool>,
    splits: SplitProgression,
    next: Option<Action>,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            loading: Watcher::new(),
            splits: SplitProgression::new(),
            next: None,
        }
    }

    pub fn act(&mut self, data: &Data<'_>) -> Option<Action> {
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

    pub fn reset(&mut self) {
        if !matches!(self.splits, SplitProgression::NotRunning { .. }) {
            *self = Progress::new();
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
