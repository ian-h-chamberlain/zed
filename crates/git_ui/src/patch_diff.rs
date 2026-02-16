use anyhow::Result;
use buffer_diff::BufferDiff;
use collections::BTreeMap;
use edit_prediction::udiff::{DiffEvent, DiffParser as UdiffParser, FileStatus};
use editor::{Editor, EditorEvent, MultiBuffer, multibuffer_context_lines};
use futures::FutureExt;
use gpui::{
    AnyElement, App, AppContext as _, AsyncApp, Context, Entity, EventEmitter, FocusHandle,
    Focusable, IntoElement, Render, SharedString, Task, Window,
};
use language::{Buffer, Capability, OffsetRangeExt};
use multi_buffer::PathKey;
use project::Project;
use std::{
    any::{Any, TypeId},
    path::{Path, PathBuf},
    sync::Arc,
};
use ui::{Color, Icon, IconName, Label, LabelCommon as _};
use util::{
    ResultExt,
    paths::PathStyle,
    rel_path::{RelPath, RelPathBuf},
};
use workspace::{
    Item, ItemHandle as _, ItemNavHistory, ToolbarItemLocation, Workspace,
    item::{BreadcrumbText, ItemEvent, SaveOptions, TabContentParams},
    searchable::SearchableItemHandle,
};
use zed_actions::git::PatchFileDiff;

pub struct PatchDiffView {
    editor: Entity<Editor>,
    file_count: usize,
}

async fn load_entries_from_patch<'a>(
    patch_content: String,
    project: &Entity<Project>,
    cx: &mut AsyncApp,
) -> Result<BTreeMap<PathBuf, (Entity<Buffer>, Entity<BufferDiff>)>> {
    let mut buffer_contents = BTreeMap::new();
    let mut patched_files = BTreeMap::new();

    // Trim off RFC 3676 signature (like `git format-patch` produces).
    // Could consider moving this logic into udiff parser, but maybe less risky here
    let patch_content = if let Some(trailing_sig) = patch_content.rfind("-- \n") {
        &patch_content[..trailing_sig]
    } else {
        &patch_content
    };

    // Parse the patch content into individual file entries
    let mut diff = UdiffParser::new(patch_content);

    let mut cur_file = None;
    while let Ok(Some(evt)) = diff.next() {
        match evt {
            DiffEvent::Hunk {
                path,
                mut hunk,
                status,
            } => match status {
                FileStatus::Modified => {
                    log::debug!("file {path:?} modified hunk: {hunk:#?}");
                    let (old_contents, new_contents) = buffer_contents
                        .entry(path.clone())
                        .or_insert_with(|| (String::new(), String::new()));

                    let start_line = hunk.start_line.unwrap_or_default() as usize;

                    let missing_lines = start_line.saturating_sub(old_contents.lines().count());

                    old_contents.push_str(&"\n".repeat(missing_lines + 1));
                    old_contents.push_str(&hunk.context);

                    new_contents.push_str(&"\n".repeat(missing_lines + 1));

                    let mut char_offset = 0isize;
                    for mut edit in hunk.edits {
                        edit.range.start = edit.range.start.saturating_add_signed(char_offset);
                        edit.range.end = edit.range.end.saturating_add_signed(char_offset);
                        char_offset = char_offset
                            .saturating_add_unsigned(edit.text.len())
                            .saturating_sub_unsigned(edit.range.len());

                        hunk.context.replace_range(edit.range, &edit.text);
                    }
                    new_contents.push_str(&hunk.context);
                    cur_file = Some(path);
                }
                FileStatus::Created => {
                    buffer_contents.insert(
                        path,
                        (
                            String::new(),
                            hunk.edits.into_iter().map(|edit| edit.text).collect(),
                        ),
                    );
                }
                FileStatus::Deleted => {
                    buffer_contents.insert(path, (hunk.context, String::new()));
                }
            },
            DiffEvent::FileEnd { renamed_to } => {
                if let Some(new_name) = renamed_to
                    && let Some(path) = cur_file.take()
                    && let Some(entry) = buffer_contents.remove(&path)
                {
                    buffer_contents.insert(new_name, entry);
                }
            }
        }
    }

    for (path, (old_text, new_text)) in buffer_contents {
        let new_text = new_text.clone();

        let language = project.read_with(cx, |project, cx| {
            project
                .languages()
                .load_language_for_file_path(Path::new(path.as_ref()))
                .now_or_never()
                .and_then(|result| result.log_err())
        });

        let buffer = project.update(cx, |project, cx| {
            project.buffer_store().update(cx, |buffer_store, cx| {
                buffer_store.create_local_buffer(&new_text, language.clone(), false, cx)
            })
        });

        let snapshot = buffer.read_with(cx, |buffer, _cx| buffer.snapshot());

        let diff = cx.new(|cx| BufferDiff::new(&snapshot, cx));
        let update = diff
            .update(cx, |diff, cx| {
                diff.update_diff(
                    snapshot.text.clone(),
                    Some(old_text.into()),
                    Some(false),
                    language,
                    cx,
                )
            })
            .await;

        diff.update(cx, |diff, cx| diff.set_snapshot(update, &snapshot, cx))
            .await;

        // assume -p1 for now, not sure if anything else needs to be supported
        let path = PathBuf::from(path.as_ref());
        let path = path
            .strip_prefix(path.iter().next().unwrap_or_default())
            .unwrap_or(&path);

        log::debug!("inserting {path:?} buffer diff");
        patched_files.insert(path.to_path_buf(), (buffer, diff));
    }

    Ok(patched_files)
}

fn register_entry(
    multibuffer: &Entity<MultiBuffer>,
    path: PathBuf,
    buffer: Entity<Buffer>,
    diff: Entity<BufferDiff>,
    context_lines: u32,
    cx: &mut Context<Workspace>,
) {
    let snapshot = buffer.read(cx).snapshot();
    let diff_snapshot = diff.read(cx).snapshot(cx);

    let ranges: Vec<_> = diff_snapshot
        .hunks(&snapshot)
        .map(|hunk| hunk.buffer_range.to_point(&snapshot))
        .collect();

    log::debug!("Collected ranges for diff hunks: {ranges:#?}");

    // TODO maybe sort by the order they appear in patch file
    // also not sure if windows path styles are supported in diffs but let's just assume POSIX for now
    let path_key = PathKey::with_sort_prefix(
        0,
        RelPath::new(&path, PathStyle::Posix)
            .log_err()
            .map(|path| path.to_rel_path_buf())
            .unwrap_or_else(RelPathBuf::new)
            .into(),
    );

    multibuffer.update(cx, |multibuffer, cx| {
        multibuffer.set_excerpts_for_path(path_key, buffer, ranges, context_lines, cx);
        multibuffer.add_diff(diff, cx);
    });
}

pub fn register(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
    PatchDiffView::register(workspace, cx);
}

impl PatchDiffView {
    pub fn register(workspace: &mut Workspace, _cx: &mut Context<Workspace>) {
        workspace.register_action(move |workspace, _: &PatchFileDiff, window, cx| {
            if let Some(editor) = Self::resolve_active_item_as_diff_editor(workspace, cx) {
                let buffer = editor.read(cx).buffer().read(cx);
                if let Some(buffer) = buffer.as_singleton() {
                    let patch_content = buffer.read(cx).text();
                    let task = Self::open(patch_content, workspace, window, cx);
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

    fn is_diff_file<V>(editor: &Entity<Editor>, cx: &mut Context<V>) -> bool {
        let buffer = editor.read(cx).buffer().read(cx);
        if let Some(buffer) = buffer.as_singleton()
            && let Some(language) = buffer.read(cx).language()
        {
            return language.name() == "Diff".into();
        }
        false
    }

    pub fn open(
        patch_content: String,
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>>> {
        let project = workspace.project().clone();
        let workspace = workspace.weak_handle();
        let context_lines = multibuffer_context_lines(cx);

        window.spawn(cx, async move |cx| {
            let patched_files = load_entries_from_patch(patch_content, &project, cx).await?;

            workspace.update_in(cx, |workspace, window, cx| {
                let multibuffer = cx.new(|cx| {
                    let mut multibuffer = MultiBuffer::new(Capability::ReadOnly);
                    multibuffer.set_all_diff_hunks_expanded(cx);
                    multibuffer
                });

                let file_count = patched_files.len();
                for (path, (buffer, diff)) in patched_files {
                    register_entry(&multibuffer, path, buffer, diff, context_lines, cx);
                }

                let diff_view = cx.new(|cx| {
                    Self::new(multibuffer.clone(), project.clone(), file_count, window, cx)
                });

                let pane = workspace.active_pane();
                pane.update(cx, |pane, cx| {
                    pane.add_item(Box::new(diff_view.clone()), true, true, None, window, cx);
                });

                // Hide the left dock (file explorer) for a cleaner diff view
                workspace.left_dock().update(cx, |dock, cx| {
                    dock.set_open(false, window, cx);
                });

                diff_view
            })
        })
    }

    fn new(
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

    fn title(&self) -> SharedString {
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
    ) -> gpui::Task<Result<()>> {
        self.editor
            .update(cx, |editor, cx| editor.save(options, project, window, cx))
    }
}

impl Render for PatchDiffView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.editor.clone()
    }
}
