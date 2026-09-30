use gpui_kit::component::{
    ActiveTheme, Disableable as _, Icon, IconName, Sizable as _, StyledExt as _, Theme, ThemeMode,
    WindowExt,
    button::{Button, ButtonVariant, ButtonVariants as _},
    checkbox::Checkbox,
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement as _,
    tab::TabBar,
    v_flex,
};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Active,
    Completed,
}

impl Filter {
    const ALL: [Filter; 3] = [Filter::All, Filter::Active, Filter::Completed];

    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Active => "Active",
            Filter::Completed => "Completed",
        }
    }

    fn matches(self, task: &Task) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => !task.done,
            Filter::Completed => task.done,
        }
    }
}

struct Task {
    id: u64,
    title: SharedString,
    done: bool,
}

pub struct TaskList {
    input: Entity<InputState>,
    tasks: Vec<Task>,
    next_id: u64,
    filter: Filter,
    /// Whether the theme tracks the system appearance. Choosing a theme with
    /// the toggle turns this off for the rest of the session.
    follow_system_theme: bool,
    _subscriptions: Vec<Subscription>,
}

impl TaskList {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("New task"));
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            match event {
                InputEvent::PressEnter { .. } => this.add_task(window, cx),
                // Re-render so the Add button tracks whether there is text.
                InputEvent::Change => cx.notify(),
                _ => {}
            }
        });
        input.update(cx, |state, cx| state.focus(window, cx));

        Theme::sync_system_appearance(Some(window), cx);
        let appearance = cx.observe_window_appearance(window, |this, window, cx| {
            if this.follow_system_theme {
                Theme::sync_system_appearance(Some(window), cx);
            }
        });

        let mut list = Self {
            input,
            tasks: Vec::new(),
            next_id: 0,
            filter: Filter::All,
            follow_system_theme: true,
            _subscriptions: vec![subscription, appearance],
        };
        for (title, done) in [
            ("Read the GPUI Kit design guides", true),
            ("Sketch the main window", false),
            ("Wire up keyboard shortcuts", false),
        ] {
            list.push(title.into(), done);
        }
        list
    }

    fn push(&mut self, title: SharedString, done: bool) {
        self.tasks.push(Task {
            id: self.next_id,
            title,
            done,
        });
        self.next_id += 1;
    }

    fn draft(&self, cx: &Context<Self>) -> SharedString {
        self.input.read(cx).value().trim().to_string().into()
    }

    fn add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.draft(cx);
        if title.is_empty() {
            return;
        }
        self.push(title, false);
        // A new task is active; make sure it is visible.
        if self.filter == Filter::Completed {
            self.filter = Filter::All;
        }
        self.input.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    fn set_done(&mut self, id: u64, done: bool, cx: &mut Context<Self>) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) {
            task.done = done;
            cx.notify();
        }
    }

    fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        self.tasks.retain(|task| task.id != id);
        cx.notify();
    }

    fn clear_completed(&mut self, cx: &mut Context<Self>) {
        self.tasks.retain(|task| !task.done);
        cx.notify();
    }

    fn confirm_remove(&self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(task) = self.tasks.iter().find(|task| task.id == id) else {
            return;
        };
        let title: SharedString = format!("Delete “{}”?", task.title).into();
        let list = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let list = list.clone();
            dialog
                .confirm()
                .title(title.clone())
                .description("This task will be removed permanently.")
                .ok_text("Delete")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("Cancel")
                .on_ok(move |_, _, cx| {
                    list.update(cx, |list, cx| list.remove(id, cx)).ok();
                    true
                })
        });
    }

    fn toggle_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = if cx.theme().is_dark() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        self.follow_system_theme = false;
        Theme::change(mode, Some(window), cx);
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let done = self.tasks.iter().filter(|task| task.done).count();
        let is_dark = cx.theme().is_dark();

        h_flex()
            .gap_2()
            .child(
                v_flex()
                    .flex_1()
                    .gap_1()
                    .child(div().text_lg().font_semibold().child("Tasks"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{done} of {} done", self.tasks.len())),
                    ),
            )
            .child(
                Button::new("toggle-theme")
                    .ghost()
                    .small()
                    .icon(if is_dark {
                        IconName::Sun
                    } else {
                        IconName::Moon
                    })
                    .accessibility_label("Toggle dark mode")
                    .tooltip(if is_dark {
                        "Switch to light mode"
                    } else {
                        "Switch to dark mode"
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_theme(window, cx))),
            )
    }

    fn render_composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_draft = !self.draft(cx).is_empty();

        h_flex()
            .gap_2()
            .child(div().flex_1().child(Input::new(&self.input)))
            .child(
                Button::new("add-task")
                    .icon(IconName::Plus)
                    .label("Add")
                    .disabled(!has_draft)
                    .on_click(cx.listener(|this, _, window, cx| this.add_task(window, cx))),
            )
    }

    fn render_task(&self, task: &Task, cx: &mut Context<Self>) -> impl IntoElement {
        let id = task.id;

        h_flex()
            .id(("task", id))
            .gap_2()
            .px_2()
            .py_1()
            .rounded(cx.theme().radius)
            .hover(|style| style.bg(cx.theme().muted))
            .child(
                Checkbox::new(("task-done", id))
                    .flex_1()
                    .label(task.title.clone())
                    .checked(task.done)
                    .on_change(cx.listener(move |this, done, _, cx| this.set_done(id, *done, cx))),
            )
            .child(
                Button::new(("task-delete", id))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Delete)
                    .accessibility_label(format!("Delete “{}”…", task.title))
                    .tooltip("Delete…")
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.confirm_remove(id, window, cx)),
                    ),
            )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, description) = match self.filter {
            Filter::All => ("No tasks", "Type a task above and press Enter to add it."),
            Filter::Active => ("Nothing left to do", "Every task is completed."),
            Filter::Completed => ("No completed tasks", "Check off a task to see it here."),
        };

        div().flex_1().flex().items_center().justify_center().child(
            Empty::new().header(
                EmptyHeader::new()
                    .media(
                        EmptyMedia::new().child(
                            Icon::new(IconName::Inbox).text_color(cx.theme().muted_foreground),
                        ),
                    )
                    .title(EmptyTitle::new().child(title))
                    .description(EmptyDescription::new().child(description)),
            ),
        )
    }
}

impl Render for TaskList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| self.filter.matches(task))
            .collect();
        let remaining = self.tasks.iter().filter(|task| !task.done).count();
        let has_completed = remaining < self.tasks.len();
        let selected = Filter::ALL
            .iter()
            .position(|filter| *filter == self.filter)
            .unwrap_or(0);

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                v_flex()
                    .p_4()
                    .gap_4()
                    .child(self.render_header(cx))
                    .child(self.render_composer(cx))
                    .child(
                        TabBar::new("filter")
                            .segmented()
                            .children(Filter::ALL.map(Filter::label))
                            .selected_index(selected)
                            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                                this.filter = Filter::ALL[*ix];
                                cx.notify();
                            })),
                    ),
            )
            .map(|this| {
                if visible.is_empty() {
                    this.child(self.render_empty(cx))
                } else {
                    let rows: Vec<_> = visible
                        .iter()
                        .map(|task| self.render_task(task, cx).into_any_element())
                        .collect();
                    this.child(
                        v_flex()
                            .id("task-list")
                            .flex_1()
                            .min_h_0()
                            .px_2()
                            .gap_1()
                            .overflow_y_scrollbar()
                            .children(rows),
                    )
                }
            })
            .child(
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(match remaining {
                                1 => "1 task left".to_string(),
                                n => format!("{n} tasks left"),
                            }),
                    )
                    .child(
                        Button::new("clear-completed")
                            .outline()
                            .small()
                            .label("Clear completed")
                            .disabled(!has_completed)
                            .on_click(cx.listener(|this, _, _, cx| this.clear_completed(cx))),
                    ),
            )
    }
}
