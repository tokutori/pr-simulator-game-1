const MAX_GENERATION: u64 = 9_007_199_254_740_991;

/// Semantic identity of one spatial menu opening.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuScope {
    /// Published panel identifier.
    pub panel_id: String,
    /// Opaque semantic identity supplied by the shared view.
    pub view_key: String,
}

impl MenuScope {
    fn valid(&self) -> bool {
        valid_id(&self.panel_id) && valid_id(&self.view_key)
    }
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024
}

/// Revision of the menu targeted by an input observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuContext {
    /// Semantic opening identity.
    pub scope: MenuScope,
    /// Monotonic input/layout revision.
    pub generation: u64,
}

/// Focus within the current menu revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuFocus {
    /// No control has focus.
    None,
    /// Focused published control.
    Control {
        /// Published control identifier.
        control_id: String,
    },
}

/// Observation of menu opening or closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    /// No spatial menu is presented.
    Closed,
    /// Current spatial menu.
    Active {
        /// Semantic identity derived from the presented view.
        scope: MenuScope,
    },
}

/// Relative direction of a page action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuPageDirection {
    /// Move toward the beginning.
    Previous,
    /// Move toward the end.
    Next,
}

/// Scroll observation expressed in viewport units or normalized progress.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuScrollIntent {
    /// Page button action.
    Page {
        /// Direction of travel.
        direction: MenuPageDirection,
        /// One viewport's fraction of the scrollable extent.
        page_progress: f64,
    },
    /// Continuous scroll input.
    Delta {
        /// Signed number of viewport heights.
        viewport_pages: f64,
        /// One viewport's fraction of the scrollable extent.
        page_progress: f64,
    },
    /// Explicit normalized progress.
    SetProgress {
        /// Value in the closed unit interval.
        progress: f64,
    },
}

/// Input protocol of the engine-independent menu owner.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuSessionInput {
    /// Synchronizes the presented opening against the observed revision.
    MenuScrollSynchronized {
        /// Revision observed by the caller.
        generation: u64,
        /// Current presentation target.
        target: MenuTarget,
    },
    /// Applies scrolling to the matching opening and revision.
    MenuScroll {
        /// Observed input context.
        context: MenuContext,
        /// Raw scroll intent.
        intent: MenuScrollIntent,
    },
    /// Updates focus within the matching input context.
    MenuFocus {
        /// Observed input context.
        context: MenuContext,
        /// Current focus.
        focus: MenuFocus,
    },
    /// Retires input after a layout or font change.
    MenuScrollInvalidated {
        /// Context whose layout changed.
        context: MenuContext,
    },
}

/// Read-only projection of the menu owner's current state.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuSessionState {
    /// Closed menu retaining its lifetime revision.
    Closed {
        /// Monotonic revision.
        generation: u64,
    },
    /// Active opening and its interaction state.
    Active {
        /// Current semantic identity.
        scope: MenuScope,
        /// Current input/layout revision.
        generation: u64,
        /// Scroll position in the closed unit interval.
        progress: f64,
        /// Current focus.
        focus: MenuFocus,
    },
}

impl Default for MenuSessionState {
    fn default() -> Self {
        Self::Closed { generation: 0 }
    }
}

impl MenuSessionState {
    /// Returns the current monotonic revision.
    pub fn generation(&self) -> u64 {
        match self {
            Self::Closed { generation } | Self::Active { generation, .. } => *generation,
        }
    }

    /// Checks whether an input belongs to the current opening and revision.
    pub fn accepts(&self, context: &MenuContext) -> bool {
        matches!(self, Self::Active { scope, generation, .. }
            if scope == &context.scope && *generation == context.generation)
    }
}

/// Owns scroll, focus and input revisions for one presentation lifetime.
#[derive(Default)]
pub struct MenuSession {
    state: MenuSessionState,
}

impl MenuSession {
    /// Borrows the current read-only projection.
    pub fn state(&self) -> &MenuSessionState {
        &self.state
    }

    /// Admits an observation and atomically updates the interaction state.
    pub fn update(&mut self, input: MenuSessionInput) -> &MenuSessionState {
        match input {
            MenuSessionInput::MenuScrollSynchronized { generation, target } => {
                if generation != self.state.generation() {
                    return &self.state;
                }
                let next = match target {
                    MenuTarget::Closed if matches!(self.state, MenuSessionState::Active { .. }) => {
                        MenuSessionState::Closed { generation }
                    }
                    MenuTarget::Active { scope }
                        if scope.valid()
                            && !matches!(&self.state, MenuSessionState::Active { scope: current, .. } if current == &scope) =>
                    {
                        MenuSessionState::Active {
                            scope,
                            generation,
                            progress: 0.0,
                            focus: MenuFocus::None,
                        }
                    }
                    _ => return &self.state,
                };
                if generation < MAX_GENERATION {
                    self.state = match next {
                        MenuSessionState::Closed { .. } => MenuSessionState::Closed {
                            generation: generation + 1,
                        },
                        MenuSessionState::Active {
                            scope,
                            progress,
                            focus,
                            ..
                        } => MenuSessionState::Active {
                            scope,
                            generation: generation + 1,
                            progress,
                            focus,
                        },
                    };
                }
            }
            MenuSessionInput::MenuScroll { context, intent } if self.state.accepts(&context) => {
                if let MenuSessionState::Active {
                    progress,
                    generation,
                    focus,
                    ..
                } = &mut self.state
                {
                    let next = match intent {
                        MenuScrollIntent::SetProgress { progress } if unit(progress) => progress,
                        MenuScrollIntent::Page {
                            direction,
                            page_progress,
                        } if unit(page_progress) => (*progress
                            + if direction == MenuPageDirection::Next {
                                page_progress
                            } else {
                                -page_progress
                            })
                        .clamp(0.0, 1.0),
                        MenuScrollIntent::Delta {
                            viewport_pages,
                            page_progress,
                        } if unit(page_progress)
                            && viewport_pages.is_finite()
                            && (viewport_pages * page_progress).is_finite() =>
                        {
                            (*progress + viewport_pages * page_progress).clamp(0.0, 1.0)
                        }
                        _ => return &self.state,
                    };
                    if next != *progress && *generation < MAX_GENERATION {
                        *progress = next;
                        *generation += 1;
                        *focus = MenuFocus::None;
                    }
                }
            }
            MenuSessionInput::MenuFocus { context, focus } if self.state.accepts(&context) => {
                if matches!(&focus, MenuFocus::Control { control_id } if !valid_id(control_id)) {
                    return &self.state;
                }
                if let MenuSessionState::Active { focus: current, .. } = &mut self.state {
                    *current = focus;
                }
            }
            MenuSessionInput::MenuScrollInvalidated { context } if self.state.accepts(&context) => {
                if let MenuSessionState::Active {
                    generation, focus, ..
                } = &mut self.state
                    && *generation < MAX_GENERATION
                {
                    *generation += 1;
                    *focus = MenuFocus::None;
                }
            }
            _ => {}
        }
        &self.state
    }
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests;
