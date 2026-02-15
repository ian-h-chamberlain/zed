use anyhow::Result;
use buffer_diff::BufferDiff;
use edit_prediction::udiff::{DiffEvent, DiffParser as UdiffParser, FileStatus};
use editor::{Editor, EditorEvent, MultiBuffer, multibuffer_context_lines};
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
use util::paths::PathStyle;
use util::rel_path::RelPath;
use workspace::{
    Item, ItemHandle as _, ItemNavHistory, ToolbarItemLocation, Workspace,
    item::{BreadcrumbText, ItemEvent, SaveOptions, TabContentParams},
    searchable::SearchableItemHandle,
};
use zed_actions::git::PatchFileDiff;

/// Represents a parsed file from a patch
struct ParsedPatchFile {
    old_path: String,
    new_path: String,
    hunks: Vec<PatchHunk>,
    status: FileStatus,
}

// TODO: maybe can reuse Hunk, DiffLine, etc. from udiff.
// Possibly move to a new crate or put in buffer_diff or something?
/// Represents a hunk (change section) in a patch file
struct PatchHunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<PatchLine>,
}

/// Represents a line in a patch
enum PatchLine {
    Context(String),
    Addition(String),
    Deletion(String),
}

/// Parse a unified diff patch into individual file entries using udiff parser
fn parse_unified_diff(patch_content: &str) -> Result<Vec<ParsedPatchFile>> {
    let mut files = Vec::new();
    let mut diff = UdiffParser::new(patch_content);
    let mut current_file: Option<ParsedPatchFile> = None;

    while let Some(event) = diff.next()? {
        match event {
            DiffEvent::Hunk { path, hunk, status } => {
                // Get or create the current file entry
                let file = current_file.get_or_insert_with(|| ParsedPatchFile {
                    old_path: path.to_string(),
                    new_path: path.to_string(),
                    hunks: Vec::new(),
                    status,
                });

                // Build PatchHunk and lines from udiff::Hunk
                let context_lines_str: Vec<&str> = hunk.context.lines().collect();
                let mut lines = Vec::new();

                // Interleave context and edits to reconstruct original diff order
                let mut context_idx = 0;
                for edit in &hunk.edits {
                    // Add any context lines before this edit
                    while context_idx < edit.range.start {
                        if context_idx < context_lines_str.len() {
                            lines.push(PatchLine::Context(
                                context_lines_str[context_idx].to_string(),
                            ));
                        }
                        context_idx += 1;
                    }

                    // Add deletion (empty text means deletion)
                    if edit.text.is_empty() {
                        if context_idx < context_lines_str.len() {
                            lines.push(PatchLine::Deletion(
                                context_lines_str[context_idx].to_string(),
                            ));
                            context_idx += 1;
                        }
                    } else {
                        // Add addition (non-empty text)
                        for add_line in edit.text.lines() {
                            lines.push(PatchLine::Addition(add_line.to_string()));
                        }
                    }
                }

                // Add remaining context lines
                while context_idx < context_lines_str.len() {
                    lines.push(PatchLine::Context(
                        context_lines_str[context_idx].to_string(),
                    ));
                    context_idx += 1;
                }

                let context_count = context_lines_str.len();
                let deletion_count = hunk.edits.iter().filter(|e| e.text.is_empty()).count();
                let addition_count: usize = hunk
                    .edits
                    .iter()
                    .filter(|e| !e.text.is_empty())
                    .map(|e| e.text.lines().count())
                    .sum();

                let patch_hunk = PatchHunk {
                    old_start: hunk.start_line.map(|l| l as usize).unwrap_or(1),
                    old_count: context_count + deletion_count,
                    new_start: hunk.start_line.map(|l| l as usize).unwrap_or(1),
                    new_count: context_count + addition_count,
                    lines,
                };
                file.hunks.push(patch_hunk);
                file.status = status;
            }
            DiffEvent::FileEnd { renamed_to } => {
                if let Some(mut file) = current_file.take() {
                    if let Some(new_path) = renamed_to {
                        file.new_path = new_path.to_string();
                    }
                    files.push(file);
                }
            }
        }
    }

    // Handle any remaining file
    if let Some(file) = current_file {
        files.push(file);
    }

    Ok(files)
}

fn common_prefix(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut iter = paths.iter();
    let mut prefix = iter.next()?.clone();

    for path in iter {
        while !path.starts_with(&prefix) {
            if !prefix.pop() {
                return Some(PathBuf::new());
            }
        }
    }

    Some(prefix)
}

pub struct PatchDiffView {
    editor: Entity<Editor>,
    file_count: usize,
}

struct Entry {
    index: usize,
    new_path: PathBuf,
    new_buffer: Entity<Buffer>,
    diff: Entity<BufferDiff>,
}

async fn load_entries_from_patch(
    patch_content: String,
    project: &Entity<Project>,
    cx: &mut AsyncApp,
) -> Result<(Vec<Entry>, Option<PathBuf>)> {
    let mut entries = Vec::new();
    let mut all_paths = Vec::new();

    // Parse the patch content into individual file entries
    let parsed_files = parse_unified_diff(&patch_content)?;

    for (index, parsed_file) in parsed_files.into_iter().enumerate() {
        // Use the new path for the file, or fall back to old path
        let file_path = if parsed_file.status != FileStatus::Deleted {
            PathBuf::from(&parsed_file.new_path)
        } else {
            PathBuf::from(&parsed_file.old_path)
        };

        // Reconstruct the file content from the hunks
        let (buffer_content, base_content) = reconstruct_file_content(&parsed_file);

        // Create a buffer with the reconstructed file content
        let buffer = project.update(cx, |project, cx| {
            project.buffer_store().update(cx, |buffer_store, cx| {
                buffer_store.create_local_buffer(buffer_content.as_str(), None, false, cx)
            })
        });

        // Create a buffer diff for this file
        let diff = build_file_buffer_diff(&parsed_file, &buffer, base_content, cx).await?;

        all_paths.push(file_path.clone());
        entries.push(Entry {
            index,
            new_path: file_path,
            new_buffer: buffer.clone(),
            diff,
        });
    }

    let common_root = common_prefix(&all_paths);
    Ok((entries, common_root))
}

/// Reconstruct the file content from parsed patch hunks
/// Returns:
/// - buffer_content: the actual file content (new version without +/- markers)
/// - base_content: the original file content before the patch was applied
fn reconstruct_file_content(parsed_file: &ParsedPatchFile) -> (String, String) {
    let mut buffer_content = String::new();
    let mut base_content = String::new();

    // Track line numbers to properly reconstruct the file
    let mut old_line = 1;
    let mut new_line = 1;

    for hunk in &parsed_file.hunks {
        // Add context lines before the hunk starts (in the old file)
        let context_before = hunk.old_start.saturating_sub(old_line);
        if context_before > 0 {
            // We don't have the actual context lines, so we'll use empty lines
            for _ in 0..context_before {
                buffer_content.push('\n');
                base_content.push('\n');
                old_line += 1;
                new_line += 1;
            }
        }

        // Process hunk lines
        for line in &hunk.lines {
            match line {
                PatchLine::Context(content) => {
                    buffer_content.push_str(content);
                    buffer_content.push('\n');
                    base_content.push_str(content);
                    base_content.push('\n');
                    old_line += 1;
                    new_line += 1;
                }
                PatchLine::Addition(content) => {
                    // Addition: only in new file
                    buffer_content.push_str(content);
                    buffer_content.push('\n');
                    new_line += 1;
                }
                PatchLine::Deletion(content) => {
                    // Deletion: only in old file (base)
                    base_content.push_str(content);
                    base_content.push('\n');
                    old_line += 1;
                }
            }
        }
    }

    (buffer_content, base_content)
}

/// Build a buffer diff for a specific file from parsed patch data
async fn build_file_buffer_diff(
    parsed_file: &ParsedPatchFile,
    buffer: &Entity<Buffer>,
    base_content: String,
    cx: &mut AsyncApp,
) -> Result<Entity<BufferDiff>> {
    // The buffer already contains the reconstructed file content
    // (set in load_entries_from_patch)
    let buffer_snapshot = buffer.read_with(cx, |buffer, _| buffer.snapshot());

    // Create a buffer diff for this specific file
    // Pass base_content as the original file content so the diff can compute changes
    let _parsed_file = parsed_file; // Suppress unused warning for now
    let diff = cx.new(|cx| BufferDiff::new(&buffer_snapshot, cx));

    let update = diff
        .update(cx, |diff, cx| {
            diff.update_diff(
                buffer_snapshot.text.clone(),
                Some(Arc::from(base_content)),
                Some(false),
                buffer_snapshot.language().cloned(),
                cx,
            )
        })
        .await;

    diff.update(cx, |diff, cx| {
        diff.set_snapshot(update, &buffer_snapshot, cx)
    })
    .await;

    Ok(diff)
}

fn register_entry(
    multibuffer: &Entity<MultiBuffer>,
    entry: Entry,
    common_root: &Option<PathBuf>,
    context_lines: u32,
    cx: &mut Context<Workspace>,
) {
    let snapshot = entry.new_buffer.read(cx).snapshot();
    let diff_snapshot = entry.diff.read(cx).snapshot(cx);

    let ranges: Vec<std::ops::Range<language::Point>> = diff_snapshot
        .hunks(&snapshot)
        .map(|hunk| hunk.buffer_range.to_point(&snapshot))
        .collect();

    let display_rel = common_root
        .as_ref()
        .and_then(|root| entry.new_path.strip_prefix(root).ok())
        .map(|rel| {
            RelPath::new(rel, PathStyle::local())
                .map(|r| r.into_owned().into())
                .unwrap_or_else(|_| {
                    RelPath::new(Path::new("untitled"), PathStyle::Posix)
                        .unwrap()
                        .into_owned()
                        .into()
                })
        })
        .unwrap_or_else(|| {
            entry
                .new_path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|s| RelPath::new(Path::new(s), PathStyle::Posix).ok())
                .map(|r| r.into_owned().into())
                .unwrap_or_else(|| {
                    RelPath::new(Path::new("untitled"), PathStyle::Posix)
                        .unwrap()
                        .into_owned()
                        .into()
                })
        });

    let path_key = PathKey::with_sort_prefix(entry.index as u64, display_rel);

    multibuffer.update(cx, |multibuffer, cx| {
        multibuffer.set_excerpts_for_path(
            path_key,
            entry.new_buffer.clone(),
            ranges,
            context_lines,
            cx,
        );
        multibuffer.add_diff(entry.diff.clone(), cx);
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
            let (entries, common_root) =
                load_entries_from_patch(patch_content, &project, cx).await?;

            workspace.update_in(cx, |workspace, window, cx| {
                let multibuffer = cx.new(|cx| {
                    let mut multibuffer = MultiBuffer::new(Capability::ReadWrite);
                    multibuffer.set_all_diff_hunks_expanded(cx);
                    multibuffer
                });

                let file_count = entries.len();
                for entry in entries {
                    register_entry(&multibuffer, entry, &common_root, context_lines, cx);
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
