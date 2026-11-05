//! Lua dialogue callbacks.
//!
//! Core's `dialogue_system` triggers the dialogue events on the runner
//! entity; these observers — spawned only in games that run a Lua script —
//! call the entity's callback component as `fn(ctx, dialogue)`, where `ctx`
//! is the entity context and `dialogue` is a fresh table built per event:
//!
//! - [`lua_dialogue_node_entered_observer`] (on [`DialogueNodeEntered`])
//!   passes `node`, `previous` (nil on the start node) and `labels` (a
//!   1-based sequence of strings) from the event, plus `speaker` (nil when
//!   the current node has none), `text` and `choices` from the
//!   [`DialogueRunner`]. `choices` holds one entry per declared choice of
//!   the current node — 0-based `index`, `text`, `labels` and `available`
//!   (false for a locked choice) — and is empty unless the runner is active
//!   on a choices node, e.g. on a [`NoChoices`](DialogueEndReason::NoChoices)
//!   ending, where `speaker` and `text` still come from the last node.
//! - [`lua_dialogue_choice_selected_observer`] (on
//!   [`DialogueChoiceSelected`]) passes `node`, 0-based `index`, `text` and
//!   `labels` from the event.
//! - [`lua_dialogue_ended_observer`] (on [`DialogueEnded`]) passes `node`
//!   (nil when the conversation was cancelled before start) and `reason`:
//!   `"completed"`, `"cancelled"` or `"no_choices"`.
//!
//! An entity without the matching callback component is skipped.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use mlua::prelude::{Lua, LuaResult, LuaTable};

use crate::components::lua_on_dialogue::{
    LuaOnDialogueChoiceSelected, LuaOnDialogueEnded, LuaOnDialogueNodeEntered,
};
use crate::systems::lua_commands::{LuaDispatch, dispatch_ctx_call_and_drain};
use aberred_core::components::dialogue::{DialogueEndReason, DialogueRunner};
use aberred_core::events::dialogue::{DialogueChoiceSelected, DialogueEnded, DialogueNodeEntered};

/// Builds the `dialogue` table of a [`DialogueNodeEntered`] callback: `node`,
/// `previous` and `labels` from `event`; `speaker`, `text` and `choices` from
/// `runner` — nil `speaker`/`text` when the runner has no current node, and
/// `choices` populated only while it is active on a choices node (a finished
/// runner keeps its last node's `speaker`/`text` but offers no choices).
pub(crate) fn node_entered_table(
    lua: &Lua,
    event: &DialogueNodeEntered,
    runner: Option<&DialogueRunner>,
) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    table.set("node", &*event.node)?;
    if let Some(previous) = &event.previous {
        table.set("previous", &**previous)?;
    }
    table.set("labels", labels_sequence(lua, &event.labels)?)?;
    let choices = lua.create_table()?;
    if let Some(runner) = runner {
        if let Some(speaker) = runner.speaker() {
            table.set("speaker", speaker)?;
        }
        if let Some(text) = runner.text() {
            table.set("text", text)?;
        }
        for choice in runner.choices() {
            let entry = lua.create_table()?;
            entry.set("index", choice.index)?;
            entry.set("text", choice.text)?;
            entry.set("labels", labels_sequence(lua, choice.labels)?)?;
            entry.set("available", choice.available)?;
            choices.push(entry)?;
        }
    }
    table.set("choices", choices)?;
    Ok(table)
}

/// Builds the `dialogue` table of a [`DialogueChoiceSelected`] callback:
/// `node`, 0-based `index`, `text` and `labels` from `event`.
pub(crate) fn choice_selected_table(
    lua: &Lua,
    event: &DialogueChoiceSelected,
) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    table.set("node", &*event.node)?;
    table.set("index", event.index)?;
    table.set("text", &*event.text)?;
    table.set("labels", labels_sequence(lua, &event.labels)?)?;
    Ok(table)
}

/// Builds the `dialogue` table of a [`DialogueEnded`] callback: `node` (nil
/// when the conversation was cancelled before start) and `reason` —
/// `"completed"`, `"cancelled"` or `"no_choices"`.
pub(crate) fn ended_table(lua: &Lua, event: &DialogueEnded) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    if let Some(node) = &event.node {
        table.set("node", &**node)?;
    }
    let reason = match event.reason {
        DialogueEndReason::Completed => "completed",
        DialogueEndReason::Cancelled => "cancelled",
        DialogueEndReason::NoChoices => "no_choices",
    };
    table.set("reason", reason)?;
    Ok(table)
}

/// Builds a fresh 1-based Lua sequence of strings from `labels`.
fn labels_sequence(lua: &Lua, labels: &[Arc<str>]) -> LuaResult<LuaTable> {
    lua.create_sequence_from(labels.iter().map(|label| &**label))
}

/// Calls the entity's [`LuaOnDialogueNodeEntered`] callback as
/// `fn(ctx, dialogue)` when its dialogue enters a node. An entity without
/// one is skipped.
pub fn lua_dialogue_node_entered_observer(
    trigger: On<DialogueNodeEntered>,
    callbacks: Query<&LuaOnDialogueNodeEntered>,
    runners: Query<&DialogueRunner>,
    mut p: LuaDispatch,
) {
    let event = trigger.event();
    let Ok(callback) = callbacks.get(event.entity) else {
        return;
    };
    let runner = runners.get(event.entity).ok();
    dispatch_ctx_call_and_drain(
        &mut p,
        event.entity,
        &callback.callback,
        "Dialogue node entered",
        |lua| node_entered_table(lua, event, runner),
    );
}

/// Calls the entity's [`LuaOnDialogueChoiceSelected`] callback as
/// `fn(ctx, dialogue)` when the player picks a choice in its dialogue. An
/// entity without one is skipped.
pub fn lua_dialogue_choice_selected_observer(
    trigger: On<DialogueChoiceSelected>,
    callbacks: Query<&LuaOnDialogueChoiceSelected>,
    mut p: LuaDispatch,
) {
    let event = trigger.event();
    let Ok(callback) = callbacks.get(event.entity) else {
        return;
    };
    dispatch_ctx_call_and_drain(
        &mut p,
        event.entity,
        &callback.callback,
        "Dialogue choice selected",
        |lua| choice_selected_table(lua, event),
    );
}

/// Calls the entity's [`LuaOnDialogueEnded`] callback as `fn(ctx, dialogue)`
/// when its dialogue ends. An entity without one is skipped.
pub fn lua_dialogue_ended_observer(
    trigger: On<DialogueEnded>,
    callbacks: Query<&LuaOnDialogueEnded>,
    mut p: LuaDispatch,
) {
    let event = trigger.event();
    let Ok(callback) = callbacks.get(event.entity) else {
        return;
    };
    dispatch_ctx_call_and_drain(
        &mut p,
        event.entity,
        &callback.callback,
        "Dialogue ended",
        |lua| ended_table(lua, event),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_test_world;
    use aberred_core::resources::dialogue_builder::DialogueBuilder;
    use aberred_core::resources::dialogue_def::DialogueCondition;
    use aberred_core::resources::dialogue_tree::DialogueTree;
    use aberred_core::resources::world_signals::WorldSignals;
    use aberred_core::systems::dialogue::step_runner;

    /// The guard conversation: `ask` (Guard, "Pass?", label "q") offers
    /// "Yes" (label "polite") -> `yes`, "Secret" (needs flag "s") -> `yes`
    /// and "No" (end); `yes` (Guard, "Welcome") goes to `bye`, which says
    /// "Bye".
    fn test_tree() -> Arc<DialogueTree> {
        Arc::new(
            DialogueBuilder::new("ask")
                .node("ask")
                .speaker("Guard")
                .text("Pass?")
                .label("q")
                .choice("Yes", "yes")
                .label("polite")
                .choice("Secret", "yes")
                .when(DialogueCondition::flag("s"))
                .choice_end("No")
                .node("yes")
                .speaker("Guard")
                .text("Welcome")
                .goto("bye")
                .node("bye")
                .text("Bye")
                .build()
                .unwrap(),
        )
    }

    /// The `a` -> `b` goto chain.
    fn goto_tree() -> Arc<DialogueTree> {
        Arc::new(
            DialogueBuilder::new("a")
                .node("a")
                .text("A")
                .goto("b")
                .node("b")
                .text("B")
                .build()
                .unwrap(),
        )
    }

    /// The `s` -> `n` chain whose choices node `n` locks every choice.
    fn no_choices_tree() -> Arc<DialogueTree> {
        Arc::new(
            DialogueBuilder::new("s")
                .node("s")
                .text("S")
                .goto("n")
                .node("n")
                .text("N")
                .choice("x", "e")
                .when(DialogueCondition::flag("nope"))
                .choice_end("y")
                .when(DialogueCondition::flag("nope2"))
                .node("e")
                .text("E")
                .build()
                .unwrap(),
        )
    }

    /// A runner over `tree` that has entered the start node, plus the
    /// signals it stepped with.
    fn started_runner(tree: &Arc<DialogueTree>) -> (DialogueRunner, WorldSignals) {
        let mut runner = DialogueRunner::new(Arc::clone(tree));
        let mut signals = WorldSignals::default();
        step_runner(&mut runner, &mut signals);
        (runner, signals)
    }

    /// A [`DialogueNodeEntered`] for `entity` with the given fields.
    fn node_event(
        entity: Entity,
        node: &str,
        previous: Option<&str>,
        labels: &[&str],
    ) -> DialogueNodeEntered {
        DialogueNodeEntered {
            entity,
            node: Arc::from(node),
            previous: previous.map(Arc::from),
            labels: labels
                .iter()
                .map(|l| Arc::from(*l))
                .collect::<Vec<_>>()
                .into(),
        }
    }

    #[test]
    fn node_entered_table_carries_the_event_and_runner_fields() {
        let lua = Lua::new();
        let tree = test_tree();
        let (runner, _signals) = started_runner(&tree);
        let event = node_event(Entity::PLACEHOLDER, "ask", None, &["q"]);

        let table = node_entered_table(&lua, &event, Some(&runner)).expect("table");

        assert_eq!(table.get::<String>("node").expect("node"), "ask");
        assert_eq!(
            table.get::<Option<String>>("previous").expect("previous"),
            None,
            "previous is nil on the start node"
        );
        let labels: LuaTable = table.get("labels").expect("labels");
        assert_eq!(labels.get::<String>(1).expect("label 1"), "q");
        assert_eq!(
            table
                .get::<Option<String>>("speaker")
                .expect("speaker")
                .as_deref(),
            Some("Guard")
        );
        assert_eq!(table.get::<String>("text").expect("text"), "Pass?");

        let choices: LuaTable = table.get("choices").expect("choices");
        assert_eq!(choices.len().expect("choices len"), 3);
        let first: LuaTable = choices.get(1).expect("choice 1");
        assert_eq!(first.get::<i64>("index").expect("index"), 0);
        assert_eq!(first.get::<String>("text").expect("text"), "Yes");
        assert!(first.get::<bool>("available").expect("available"));
        let first_labels: LuaTable = first.get("labels").expect("labels");
        assert_eq!(first_labels.get::<String>(1).expect("label 1"), "polite");
        let locked: LuaTable = choices.get(2).expect("choice 2");
        assert_eq!(locked.get::<i64>("index").expect("index"), 1);
        assert_eq!(locked.get::<String>("text").expect("text"), "Secret");
        assert!(
            !locked.get::<bool>("available").expect("available"),
            "the locked choice is listed with available = false"
        );
        let last: LuaTable = choices.get(3).expect("choice 3");
        assert_eq!(last.get::<i64>("index").expect("index"), 2);
        assert_eq!(last.get::<String>("text").expect("text"), "No");
        assert!(last.get::<bool>("available").expect("available"));
    }

    #[test]
    fn node_entered_table_sets_previous_on_a_later_node() {
        let lua = Lua::new();
        let tree = goto_tree();
        let (mut runner, mut signals) = started_runner(&tree);
        runner.advance();
        step_runner(&mut runner, &mut signals);
        let event = node_event(Entity::PLACEHOLDER, "b", Some("a"), &[]);

        let table = node_entered_table(&lua, &event, Some(&runner)).expect("table");

        assert_eq!(table.get::<String>("previous").expect("previous"), "a");
        assert_eq!(table.get::<String>("node").expect("node"), "b");
        assert_eq!(table.get::<String>("text").expect("text"), "B");
        let choices: LuaTable = table.get("choices").expect("choices");
        assert_eq!(
            choices.len().expect("choices len"),
            0,
            "a goto node has no choices"
        );
    }

    #[test]
    fn node_entered_table_without_a_runner_has_nil_runner_fields_and_empty_choices() {
        let lua = Lua::new();
        let event = node_event(Entity::PLACEHOLDER, "ask", None, &["q"]);

        let table = node_entered_table(&lua, &event, None).expect("table");

        assert_eq!(table.get::<String>("node").expect("node"), "ask");
        assert_eq!(
            table.get::<Option<String>>("previous").expect("previous"),
            None
        );
        let labels: LuaTable = table.get("labels").expect("labels");
        assert_eq!(labels.get::<String>(1).expect("label 1"), "q");
        assert_eq!(
            table.get::<Option<String>>("speaker").expect("speaker"),
            None
        );
        assert_eq!(table.get::<Option<String>>("text").expect("text"), None);
        let choices: LuaTable = table.get("choices").expect("choices");
        assert_eq!(choices.len().expect("choices len"), 0);
    }

    #[test]
    fn node_entered_table_on_a_no_choices_finished_runner_has_empty_choices() {
        let lua = Lua::new();
        let tree = no_choices_tree();
        let (mut runner, mut signals) = started_runner(&tree);
        runner.advance();
        step_runner(&mut runner, &mut signals);
        assert!(runner.is_finished());
        assert_eq!(runner.end_reason(), Some(DialogueEndReason::NoChoices));
        let event = node_event(Entity::PLACEHOLDER, "n", Some("s"), &[]);

        let table = node_entered_table(&lua, &event, Some(&runner)).expect("table");

        let choices: LuaTable = table.get("choices").expect("choices");
        assert_eq!(
            choices.len().expect("choices len"),
            0,
            "a finished runner offers no choices"
        );
        assert_eq!(
            table.get::<String>("text").expect("text"),
            "N",
            "the last node's line is still there"
        );
        assert_eq!(
            table.get::<Option<String>>("speaker").expect("speaker"),
            None
        );
    }

    #[test]
    fn choice_selected_table_carries_the_event_fields() {
        let lua = Lua::new();
        let event = DialogueChoiceSelected {
            entity: Entity::PLACEHOLDER,
            node: Arc::from("ask"),
            index: 1,
            text: Arc::from("Secret"),
            labels: vec![Arc::from("l1"), Arc::from("l2")].into(),
        };

        let table = choice_selected_table(&lua, &event).expect("table");

        assert_eq!(table.get::<String>("node").expect("node"), "ask");
        assert_eq!(
            table.get::<i64>("index").expect("index"),
            1,
            "0-based declared index"
        );
        assert_eq!(table.get::<String>("text").expect("text"), "Secret");
        let labels: LuaTable = table.get("labels").expect("labels");
        assert_eq!(labels.get::<String>(1).expect("label 1"), "l1");
        assert_eq!(labels.get::<String>(2).expect("label 2"), "l2");
    }

    #[test]
    fn ended_table_maps_each_reason_and_a_nil_node() {
        let lua = Lua::new();
        for (reason, expected) in [
            (DialogueEndReason::Completed, "completed"),
            (DialogueEndReason::Cancelled, "cancelled"),
            (DialogueEndReason::NoChoices, "no_choices"),
        ] {
            let event = DialogueEnded {
                entity: Entity::PLACEHOLDER,
                node: Some(Arc::from("n")),
                reason,
            };
            let table = ended_table(&lua, &event).expect("table");
            assert_eq!(table.get::<String>("node").expect("node"), "n");
            assert_eq!(table.get::<String>("reason").expect("reason"), expected);
        }

        let event = DialogueEnded {
            entity: Entity::PLACEHOLDER,
            node: None,
            reason: DialogueEndReason::Cancelled,
        };
        let table = ended_table(&lua, &event).expect("table");
        assert_eq!(table.get::<Option<String>>("node").expect("node"), None);
        assert_eq!(table.get::<String>("reason").expect("reason"), "cancelled");
    }

    /// A world with the dispatch resources, a `LuaRuntime`, and the three
    /// dialogue observers, plus recording Lua callbacks.
    fn setup_world() -> World {
        let mut world = init_test_world();
        world.spawn(Observer::new(lua_dialogue_node_entered_observer));
        world.spawn(Observer::new(lua_dialogue_choice_selected_observer));
        world.spawn(Observer::new(lua_dialogue_ended_observer));
        world.flush();
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load(
                r#"
                calls = 0
                function on_node(ctx, dialogue)
                    calls = calls + 1
                    got_node = dialogue.node
                    got_previous = dialogue.previous
                end
                function on_choice(ctx, dialogue)
                    calls = calls + 1
                    got_index = dialogue.index
                    got_text = dialogue.text
                end
                function on_end(ctx, dialogue)
                    calls = calls + 1
                    got_reason = dialogue.reason
                end
                function on_node_flag(ctx, dialogue)
                    engine.set_flag('seen')
                end
                function on_node_error(ctx, dialogue)
                    engine.set_flag('pre_error')
                    error('boom')
                end
                "#,
            )
            .exec()
            .expect("lua load");
        world
    }

    fn global<T: mlua::FromLua>(world: &World, name: &str) -> T {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get(name)
            .expect(name)
    }

    #[test]
    fn node_entered_without_a_callback_is_skipped() {
        let mut world = setup_world();
        let entity = world.spawn_empty().id();

        world.trigger(node_event(entity, "ask", None, &["q"]));
        world.flush();

        assert_eq!(global::<i64>(&world, "calls"), 0);
    }

    #[test]
    fn choice_selected_without_a_callback_is_skipped() {
        let mut world = setup_world();
        let entity = world.spawn_empty().id();

        world.trigger(DialogueChoiceSelected {
            entity,
            node: Arc::from("ask"),
            index: 0,
            text: Arc::from("Yes"),
            labels: Vec::new().into(),
        });
        world.flush();

        assert_eq!(global::<i64>(&world, "calls"), 0);
    }

    #[test]
    fn ended_without_a_callback_is_skipped() {
        let mut world = setup_world();
        let entity = world.spawn_empty().id();

        world.trigger(DialogueEnded {
            entity,
            node: Some(Arc::from("ask")),
            reason: DialogueEndReason::Completed,
        });
        world.flush();

        assert_eq!(global::<i64>(&world, "calls"), 0);
    }

    #[test]
    fn node_entered_calls_the_callback_once_with_the_dialogue_table() {
        let mut world = setup_world();
        let tree = test_tree();
        let (runner, _signals) = started_runner(&tree);
        let entity = world
            .spawn((LuaOnDialogueNodeEntered::new("on_node"), runner))
            .id();

        world.trigger(node_event(entity, "ask", None, &["q"]));
        world.flush();

        assert_eq!(global::<i64>(&world, "calls"), 1);
        assert_eq!(global::<String>(&world, "got_node"), "ask");
        assert_eq!(global::<Option<String>>(&world, "got_previous"), None);
    }

    #[test]
    fn choice_selected_calls_the_callback_once_with_the_event_fields() {
        let mut world = setup_world();
        let entity = world
            .spawn(LuaOnDialogueChoiceSelected::new("on_choice"))
            .id();

        world.trigger(DialogueChoiceSelected {
            entity,
            node: Arc::from("ask"),
            index: 1,
            text: Arc::from("Secret"),
            labels: vec![Arc::from("l")].into(),
        });
        world.flush();

        assert_eq!(global::<i64>(&world, "calls"), 1);
        assert_eq!(global::<i64>(&world, "got_index"), 1);
        assert_eq!(global::<String>(&world, "got_text"), "Secret");
    }

    #[test]
    fn ended_calls_the_callback_once_with_the_reason() {
        let mut world = setup_world();
        let entity = world.spawn(LuaOnDialogueEnded::new("on_end")).id();

        world.trigger(DialogueEnded {
            entity,
            node: Some(Arc::from("ask")),
            reason: DialogueEndReason::Completed,
        });
        world.flush();

        assert_eq!(global::<i64>(&world, "calls"), 1);
        assert_eq!(global::<String>(&world, "got_reason"), "completed");
    }

    #[test]
    fn node_entered_callback_commands_are_drained() {
        let mut world = setup_world();
        let entity = world
            .spawn(LuaOnDialogueNodeEntered::new("on_node_flag"))
            .id();

        world.trigger(node_event(entity, "ask", None, &[]));
        world.flush();

        assert!(world.resource::<WorldSignals>().has_flag("seen"));
    }

    #[test]
    fn node_entered_callback_error_still_drains_queued_commands() {
        let mut world = setup_world();
        let entity = world
            .spawn(LuaOnDialogueNodeEntered::new("on_node_error"))
            .id();

        world.trigger(node_event(entity, "ask", None, &[]));
        world.flush();

        assert!(world.resource::<WorldSignals>().has_flag("pre_error"));
    }
}
