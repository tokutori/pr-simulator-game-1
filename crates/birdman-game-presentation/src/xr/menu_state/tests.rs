use super::*;

fn scope(overlay: Option<&str>) -> MenuScope {
    match overlay {
        None => MenuScope::Scene {
            scene: GameUiScene::Title,
            panel_id: "menu".into(),
            view_key: "main".into(),
        },
        Some(overlay) => MenuScope::Overlay {
            scene: GameUiScene::Title,
            panel_id: "menu".into(),
            view_key: "main".into(),
            overlay: overlay.into(),
        },
    }
}

fn context(owner: &MenuSession) -> MenuContext {
    let MenuSessionState::Active {
        scope, generation, ..
    } = owner.state()
    else {
        panic!("Expected active menu");
    };
    MenuContext {
        scope: scope.clone(),
        generation: *generation,
    }
}

fn open(owner: &mut MenuSession, scope: MenuScope) {
    owner.update(MenuSessionInput::MenuScrollSynchronized {
        generation: owner.state().generation(),
        target: MenuTarget::Active { scope },
    });
}

#[test]
fn opening_scroll_focus_and_layout_changes_retire_stale_contexts() {
    let mut owner = MenuSession::default();
    open(&mut owner, scope(None));
    let opening = context(&owner);
    owner.update(MenuSessionInput::MenuFocus {
        context: opening.clone(),
        focus: MenuFocus::Control {
            control_id: "start".into(),
        },
    });
    owner.update(MenuSessionInput::MenuScroll {
        context: opening.clone(),
        intent: MenuScrollIntent::Page {
            direction: MenuPageDirection::Next,
            page_progress: 0.25,
        },
    });
    assert!(matches!(
        owner.state(),
        MenuSessionState::Active {
            generation: 2,
            progress: 0.25,
            focus: MenuFocus::None,
            ..
        }
    ));
    let scrolled = owner.state().clone();
    for input in [
        MenuSessionInput::MenuFocus {
            context: opening.clone(),
            focus: MenuFocus::Control {
                control_id: "old".into(),
            },
        },
        MenuSessionInput::MenuScroll {
            context: opening.clone(),
            intent: MenuScrollIntent::SetProgress { progress: 1.0 },
        },
        MenuSessionInput::MenuScrollInvalidated { context: opening },
        MenuSessionInput::MenuScrollSynchronized {
            generation: 1,
            target: MenuTarget::Closed,
        },
    ] {
        assert_eq!(owner.update(input), &scrolled);
    }
    let current = context(&owner);
    owner.update(MenuSessionInput::MenuFocus {
        context: current.clone(),
        focus: MenuFocus::Control {
            control_id: "current".into(),
        },
    });
    owner.update(MenuSessionInput::MenuScrollInvalidated { context: current });
    assert!(matches!(
        owner.state(),
        MenuSessionState::Active {
            generation: 3,
            progress: 0.25,
            focus: MenuFocus::None,
            ..
        }
    ));
    for overlay in ["Settings", "Help", "Credits"] {
        open(&mut owner, scope(Some(overlay)));
        assert!(matches!(
            owner.state(),
            MenuSessionState::Active {
                progress: 0.0,
                focus: MenuFocus::None,
                ..
            }
        ));
    }
    let generation = owner.state().generation();
    owner.update(MenuSessionInput::MenuScrollSynchronized {
        generation,
        target: MenuTarget::Closed,
    });
    assert_eq!(
        owner.state(),
        &MenuSessionState::Closed {
            generation: generation + 1
        }
    );
    open(&mut owner, scope(None));
    assert_eq!(owner.state().generation(), generation + 2);
}

#[test]
fn viewport_units_clamp_progress_and_reject_invalid_inputs_atomically() {
    let mut owner = MenuSession::default();
    open(&mut owner, scope(None));
    for (pages, expected) in [(1.5, 0.375), (100.0, 1.0), (-100.0, 0.0)] {
        let current = context(&owner);
        owner.update(MenuSessionInput::MenuScroll {
            context: current,
            intent: MenuScrollIntent::Delta {
                viewport_pages: pages,
                page_progress: 0.25,
            },
        });
        assert!(
            matches!(owner.state(), MenuSessionState::Active { progress, .. } if *progress == expected)
        );
    }
    let before = owner.state().clone();
    let current = context(&owner);
    for value in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert_eq!(
            owner.update(MenuSessionInput::MenuScroll {
                context: current.clone(),
                intent: MenuScrollIntent::SetProgress { progress: value }
            }),
            &before
        );
        assert_eq!(
            owner.update(MenuSessionInput::MenuScroll {
                context: current.clone(),
                intent: MenuScrollIntent::Page {
                    direction: MenuPageDirection::Next,
                    page_progress: value
                }
            }),
            &before
        );
    }
    assert_eq!(
        owner.update(MenuSessionInput::MenuScroll {
            context: current,
            intent: MenuScrollIntent::Delta {
                viewport_pages: f64::INFINITY,
                page_progress: 0.5
            }
        }),
        &before
    );
    open(
        &mut owner,
        MenuScope::Scene {
            scene: GameUiScene::Title,
            panel_id: " ".into(),
            view_key: "main".into(),
        },
    );
    assert_eq!(owner.state(), &before);
}

#[test]
fn saturated_revision_and_repeated_observations_preserve_state() {
    let mut owner = MenuSession::default();
    open(&mut owner, scope(None));
    let before = owner.state().clone();
    open(&mut owner, scope(None));
    assert_eq!(owner.state(), &before);
    owner.state = MenuSessionState::Active {
        scope: scope(None),
        generation: MAX_GENERATION,
        progress: 0.5,
        focus: MenuFocus::None,
    };
    let saturated = owner.state().clone();
    owner.update(MenuSessionInput::MenuScroll {
        context: context(&owner),
        intent: MenuScrollIntent::SetProgress { progress: 1.0 },
    });
    assert_eq!(owner.state(), &saturated);
    owner.update(MenuSessionInput::MenuScrollSynchronized {
        generation: MAX_GENERATION,
        target: MenuTarget::Closed,
    });
    assert_eq!(owner.state(), &saturated);
}
