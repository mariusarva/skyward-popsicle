//! The modules the phone's own system loads, and switching them on or off.

use crate::app::state::ModuleRow;
use crate::app::App;

impl App {
    /// What the page shows: the modules read, each with its switch.
    pub fn modules(&self) -> Option<&Result<Vec<ModuleRow>, String>> {
        self.state.modules.as_ref()
    }

    /// The modules read for the page, or why they could not be.
    pub fn set_modules(&mut self, rows: Result<Vec<ModuleRow>, String>) {
        self.state.set_modules(rows);
        self.refresh();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::testing::*;
    use crate::app::{Action, App, Confirm, Effect, Screen};

    fn row(id: &str, name: &str, enabled: bool) -> crate::app::state::ModuleRow {
        crate::app::state::ModuleRow {
            id: id.into(),
            name: name.into(),
            note: "v1.0 · someone".into(),
            about: "what it does".into(),
            enabled,
            removing: false,
        }
    }

    #[test]
    fn the_page_lists_what_was_read_and_switching_one_asks_for_the_other_state() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Modules, W, H, PPD);
        app.state.modules = Some(Ok(vec![row("a", "Module A", true), row("b", "Module B", false)]));
        app.refresh();
        // 点一行打开它的详情，详情里才有开关与卸载。
        assert!(tap(&mut app, &f, &i, Action::ModuleOpen(0)).is_empty());
        assert_eq!(app.screen(), Screen::ModuleInfo);
        assert_eq!(tap(&mut app, &f, &i, Action::ModuleToggle(0)), vec![Effect::ModuleSwitch("a".into(), false)]);
        // 卸载现在先立一张滑动确认卡，滑动才真做。
        assert!(tap(&mut app, &f, &i, Action::ModuleRemove(0)).is_empty());
        assert_eq!(app.state.module_confirm, Some(0), "the card is up");
        app.frame(&f, &i); // 卡片画一遍，滑动条的位置才有
        assert_eq!(swipe(&mut app, &f, &i, Confirm::DeleteModule), vec![Effect::ModuleRemove("a".into(), true)]);
    }

    #[test]
    fn a_page_not_read_yet_says_so_and_switches_nothing() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Modules, W, H, PPD);
        app.state.modules = None;
        app.refresh();
        let hits = app.frame(&f, &i).hits.clone();
        assert!(hits.iter().all(|h| !matches!(h.action, Action::ModuleOpen(_))), "nothing to open");
    }
}
