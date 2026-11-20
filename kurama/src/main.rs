//! Kurama — a Rune desktop UI for the existing Omawarden vault backend.
//!
//! The app follows the shape of `rune/examples/hello`: state, a pure view, and
//! an AppSpec. Backend calls run on a worker thread and arrive via `poll`.

#![forbid(unsafe_code)]

mod backend;
mod state;
mod theme;
mod ui;

use rune::AppSpec;
use state::State;

fn main() {
    rune::run(AppSpec {
        min_size: Some((900.0, 580.0)),
        scenes: &[
            ("populated", state::scene_populated),
            ("locked", state::scene_locked),
            ("disconnected", state::scene_disconnected),
        ],
        setup: Some(state::setup),
        poll: Some(state::poll),
        ..AppSpec::new("Kurama", (1160.0, 760.0), State::default, ui::view)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn runtime(scene: fn(&mut State)) -> Runtime<State> {
        let mut state = State::default();
        scene(&mut state);
        Runtime::new(
            state,
            ui::view,
            (1160.0, 760.0),
            Env { system_dark: true, scale: 1.0 },
        )
    }

    #[test]
    fn vault_shell_has_search_sidebar_list_and_details() {
        let mut rt = runtime(state::scene_populated);
        rt.frame(1.0 / 60.0);
        for key in ["root", "search", "sidebar", "items", "item-1", "details", "status-bar"] {
            assert!(rt.find(key).is_some(), "missing key: {key}");
        }
    }

    #[test]
    fn locked_state_hides_the_vault() {
        let mut rt = runtime(state::scene_locked);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("placeholder-title").is_some());
        assert!(rt.find("item-1").is_none());
    }
}
