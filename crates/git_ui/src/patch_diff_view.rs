use std::any::{Any, TypeId};
use std::sync::Arc;

use anyhow::Result;

use editor::{Editor, EditorEvent, MultiBuffer, multibuffer_context_lines};
use gpui::prelude::*;
use gpui::{Entity, EventEmitter, FocusHandle, Focusable, Task};
use language::{Buffer, Capability};
use project::Project;
use ui::{AnyElement, App, Color, Icon, IconName, Label, LabelCommon as _, SharedString, Window};
use workspace::item::{BreadcrumbText, ItemEvent, SaveOptions, TabContentParams};
use workspace::searchable::SearchableItemHandle;
use workspace::{Item, ItemHandle as _, ItemNavHistory, ToolbarItemLocation, Workspace};
use zed_actions::git::PatchFileDiff;

use super::patch_diff;

pub struct PatchDiffView {
    editor: Entity<Editor>,
    file_count: usize,
}

impl PatchDiffView {
    pub fn register(workspace: &mut Workspace, _cx: &mut Context<Workspace>) {
        workspace.register_action(move |workspace, _: &PatchFileDiff, window, cx| {
            if let Some(editor) = Self::resolve_active_item_as_diff_editor(workspace, cx) {
                let buffer = editor.read(cx).buffer().read(cx);
                if let Some(buffer) = buffer.as_singleton() {
                    let task = Self::open(buffer, workspace, window, cx);
                    task.detach();
                }
            }
        });
    }

    pub fn resolve_active_item_as_diff_editor(
        workspace: &Workspace,
        cx: &mut Context<Workspace>,
    ) -> Option<Entity<Editor>> {
        if let Some(editor) = workspace
            .active_item(cx)
            .and_then(|item| item.act_as::<Editor>(cx))
            && Self::is_diff_file(&editor, cx)
        {
            return Some(editor);
        }
        None
    }

    pub(crate) fn is_diff_file<V>(editor: &Entity<Editor>, cx: &mut Context<V>) -> bool {
        let buffer = editor.read(cx).buffer().read(cx);
        if let Some(buffer) = buffer.as_singleton()
            && let Some(language) = buffer.read(cx).language()
        {
            return language.name() == "Diff".into();
        }
        false
    }

    pub fn open(
        patch_buffer: Entity<Buffer>,
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>>> {
        let project = workspace.project().clone();
        let workspace = workspace.weak_handle();
        let context_lines = multibuffer_context_lines(cx);
        let patch_content = patch_buffer.read(cx).text();
        let patch_filename = patch_buffer
            .read(cx)
            .file()
            .map(|file| file.file_name(cx).to_string());

        window.spawn(cx, async move |cx| {
            let patched_files = patch_diff::load_entries(patch_content, &project, cx).await?;

            workspace.update_in(cx, |workspace, window, cx| {
                let multibuffer = cx.new(|cx| {
                    let mut multibuffer = MultiBuffer::new(Capability::ReadOnly);
                    multibuffer.set_all_diff_hunks_expanded(cx);
                    multibuffer.set_use_extended_diff_range(false, cx);

                    if let Some(filename) = patch_filename {
                        multibuffer.with_title(filename)
                    } else {
                        multibuffer
                    }
                });

                let file_count = patched_files.len();
                for (path, (buffer, diff)) in patched_files {
                    patch_diff::register_entry(&multibuffer, path, buffer, diff, context_lines, cx);
                }

                let diff_view = cx.new(|cx| {
                    Self::new(multibuffer.clone(), project.clone(), file_count, window, cx)
                });

                let pane = workspace.active_pane();
                pane.update(cx, |pane, cx| {
                    pane.add_item(Box::new(diff_view.clone()), true, true, None, window, cx);
                });

                diff_view
            })
        })
    }

    pub(crate) fn new(
        multibuffer: Entity<MultiBuffer>,
        project: Entity<Project>,
        file_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| {
            // TODO: SplittableEditor so we can view side-by-side
            let mut editor =
                Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx);
            editor.start_temporary_diff_override();
            editor.disable_diagnostics(cx);
            editor.set_expand_all_diff_hunks(cx);
            editor.set_render_diff_hunk_controls(
                Arc::new(|_, _, _, _, _, _, _, _| gpui::Empty.into_any_element()),
                cx,
            );
            editor
        });

        Self { editor, file_count }
    }

    pub(crate) fn title(&self) -> SharedString {
        let suffix = if self.file_count == 1 {
            "1 file".to_string()
        } else {
            format!("{} files", self.file_count)
        };
        format!("Patch Diff ({suffix})").into()
    }
}

impl EventEmitter<EditorEvent> for PatchDiffView {}

impl Focusable for PatchDiffView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}

impl Item for PatchDiffView {
    type Event = EditorEvent;

    fn tab_icon(&self, _window: &Window, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::Diff).color(Color::Muted))
    }

    fn tab_content(&self, params: TabContentParams, _window: &Window, _cx: &App) -> AnyElement {
        Label::new(self.title())
            .color(if params.selected {
                Color::Default
            } else {
                Color::Muted
            })
            .into_any_element()
    }

    fn tab_tooltip_text(&self, _cx: &App) -> Option<ui::SharedString> {
        Some(self.title())
    }

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        self.title()
    }

    fn to_item_events(event: &EditorEvent, f: impl FnMut(ItemEvent)) {
        Editor::to_item_events(event, f)
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("Patch Diff View Opened")
    }

    fn deactivated(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .update(cx, |editor, cx| editor.deactivated(window, cx));
    }

    fn act_as_type<'a>(
        &'a self,
        type_id: TypeId,
        self_handle: &'a Entity<Self>,
        _: &'a App,
    ) -> Option<gpui::AnyEntity> {
        if type_id == TypeId::of::<Self>() {
            Some(self_handle.clone().into())
        } else if type_id == TypeId::of::<Editor>() {
            Some(self.editor.clone().into())
        } else {
            None
        }
    }

    fn as_searchable(&self, _: &Entity<Self>, _: &App) -> Option<Box<dyn SearchableItemHandle>> {
        Some(Box::new(self.editor.clone()))
    }

    fn set_nav_history(
        &mut self,
        nav_history: ItemNavHistory,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |editor, _| {
            editor.set_nav_history(Some(nav_history));
        });
    }

    fn navigate(
        &mut self,
        data: Arc<dyn Any + Send>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.editor
            .update(cx, |editor, cx| editor.navigate(data, window, cx))
    }

    fn breadcrumb_location(&self, _: &App) -> ToolbarItemLocation {
        ToolbarItemLocation::PrimaryLeft
    }

    fn breadcrumbs(&self, cx: &App) -> Option<Vec<BreadcrumbText>> {
        self.editor.breadcrumbs(cx)
    }

    fn added_to_workspace(
        &mut self,
        workspace: &mut Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |editor, cx| {
            editor.added_to_workspace(workspace, window, cx)
        });
    }

    fn can_save(&self, cx: &App) -> bool {
        self.editor.read(cx).can_save(cx)
    }

    fn save(
        &mut self,
        options: SaveOptions,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Task<anyhow::Result<()>> {
        self.editor
            .update(cx, |editor, cx| editor.save(options, project, window, cx))
    }
}

impl Render for PatchDiffView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.editor.clone()
    }
}
