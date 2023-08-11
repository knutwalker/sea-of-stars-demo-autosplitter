use asr::user_settings::Settings;

use crate::progress::{Action, Split};

#[derive(Debug, Settings)]
pub struct Settings {
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
    /// Stop game timer during loads
    stop_when_loading: bool,
}

impl Settings {
    pub fn filter(&self, action: Action) -> Option<Action> {
        Some(action).filter(|action| match action {
            Action::ResetAndStart => true,
            Action::Split(split) => match split {
                Split::Mountain => self.mountain,
                Split::Town => self.town,
                Split::Mob => self.mob,
                Split::LevelUp => self.level_up,
                Split::Dungeon => self.dungeon,
                Split::Boss => true,
            },
            Action::Pause | Action::Resume => self.stop_when_loading,
        })
    }
}
