use anyhow::Result;
use buffer_diff::BufferDiff;
use collections::BTreeMap;
use edit_prediction::udiff::{DiffEvent, DiffParser as UdiffParser, FileStatus};
use editor::{MultiBuffer, multibuffer_context_lines};
use gpui::{App, AppContext as _, AsyncApp, Context, Entity};
use language::{Buffer, Capability, DiskState, proto};
use multi_buffer::PathKey;
use project::Project;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use util::{
    ResultExt,
    paths::PathStyle,
    rel_path::{RelPath, RelPathBuf},
};
use workspace::Workspace;

pub struct PatchedFileDiff {
    buffer: Entity<Buffer>,
    diff: Entity<BufferDiff>,
    hunk_context_sizes: BTreeMap<usize, usize>,
}

pub async fn load_entries<'a>(
    patch_content: String,
    project: &Entity<Project>,
    cx: &mut AsyncApp,
) -> Result<BTreeMap<PathBuf, PatchedFileDiff>> {
    let mut buffer_contents = BTreeMap::new();
    let mut patched_files = BTreeMap::new();

    // Trim off RFC 3676 signature (like `git format-patch` produces).
    // Could consider moving this logic into udiff parser, but maybe less risky here
    let patch_content = if let Some(trailing_sig) = patch_content.rfind("-- \n") {
        &patch_content[..trailing_sig]
    } else {
        &patch_content
    };

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
                    let (old_contents, new_contents, hunk_context_sizes) = buffer_contents
                        .entry(path.clone())
                        .or_insert_with(|| (String::new(), String::new(), BTreeMap::new()));

                    let start_line = hunk.start_line.unwrap_or_default() as usize;
                    let missing_lines = start_line.saturating_sub(old_contents.lines().count());

                    old_contents.push_str(&"\n".repeat(missing_lines));
                    old_contents.push_str(&hunk.context);

                    let mut edit_line_count = 0;

                    new_contents.push_str(&"\n".repeat(missing_lines));
                    let mut char_offset = 0isize;
                    for mut edit in hunk.edits {
                        edit.range.start = edit.range.start.saturating_add_signed(char_offset);
                        edit.range.end = edit.range.end.saturating_add_signed(char_offset);
                        char_offset = char_offset
                            .saturating_add_unsigned(edit.text.len())
                            .saturating_sub_unsigned(edit.range.len());

                        hunk.context.replace_range(edit.range, &edit.text);
                        edit_line_count += edit.text.lines().count();
                    }
                    new_contents.push_str(&hunk.context);

                    let context_size = hunk.context.lines().count().saturating_sub(edit_line_count);
                    hunk_context_sizes.insert(start_line, context_size);

                    cur_file = Some(path);
                }
                FileStatus::Created => {
                    buffer_contents.insert(
                        path,
                        (
                            String::new(),
                            hunk.edits.into_iter().map(|edit| edit.text).collect(),
                            BTreeMap::new(),
                        ),
                    );
                }
                FileStatus::Deleted => {
                    buffer_contents.insert(path, (hunk.context, String::new(), BTreeMap::new()));
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

    for (path, (old_text, new_text, hunk_context_sizes)) in buffer_contents {
        let new_text = new_text.clone();

        let language_registry = project.read_with(cx, |project, _cx| project.languages().clone());
        let language = language_registry
            .load_language_for_file_path(Path::new(path.as_ref()))
            .await
            .log_err();

        let buffer = project.update(cx, |project, cx| {
            project.buffer_store().update(cx, |buffer_store, cx| {
                buffer_store.create_local_buffer(&new_text, language.clone(), false, cx)
            })
        });

        let snapshot = buffer.update(cx, |buffer, cx| {
            buffer.set_capability(Capability::ReadOnly, cx);
            // set the filename with a stubbed out File implementation
            buffer.file_updated(
                Arc::new(PatchedFile {
                    path: RelPath::new(&PathBuf::from(path.as_ref()), PathStyle::local())
                        .log_err()
                        .map(|path| path.to_rel_path_buf())
                        .unwrap_or_else(RelPathBuf::new)
                        .into(),
                    was_deleted: new_text.is_empty(),
                }) as _,
                cx,
            );
            buffer.snapshot()
        });

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
        patched_files.insert(
            path.to_path_buf(),
            PatchedFileDiff {
                buffer,
                diff,
                hunk_context_sizes,
            },
        );
    }

    Ok(patched_files)
}

pub fn register_entry(
    multibuffer: &Entity<MultiBuffer>,
    path: PathBuf,
    patched_file: PatchedFileDiff,
    cx: &mut Context<Workspace>,
) {
    let path_key = PathKey::with_sort_prefix(
        0,
        RelPath::new(&path, PathStyle::local())
            .log_err()
            .map(|path| path.to_rel_path_buf())
            .unwrap_or_else(RelPathBuf::new)
            .into(),
    );

    let snapshot = patched_file.buffer.read(cx).snapshot();
    let diff_snapshot = patched_file.diff.read(cx).snapshot(cx);
    let ranges: Vec<_> = diff_snapshot
        .hunks(&snapshot)
        .map(|hunk| hunk.range)
        .collect();

    log::debug!("Collected ranges for diff hunks in {path:?}: {ranges:#?}");

    multibuffer.update(cx, |multibuffer, cx| {
        multibuffer.set_excerpts_for_path(
            path_key,
            patched_file.buffer,
            ranges,
            multibuffer_context_lines(cx),
            cx,
        );
        multibuffer.add_diff(patched_file.diff, cx);
    });
}

struct PatchedFile {
    path: Arc<RelPath>,
    was_deleted: bool,
}

impl language::File for PatchedFile {
    fn as_local(&self) -> Option<&dyn language::LocalFile> {
        None
    }

    fn disk_state(&self) -> DiskState {
        DiskState::Historic {
            was_deleted: self.was_deleted,
        }
    }

    fn path(&self) -> &Arc<RelPath> {
        &self.path
    }

    fn full_path(&self, _cx: &App) -> PathBuf {
        self.path.as_std_path().to_path_buf()
    }

    fn path_style(&self, _cx: &App) -> PathStyle {
        PathStyle::local()
    }

    fn file_name<'a>(&'a self, _cx: &'a App) -> &'a str {
        self.path.file_name().unwrap_or_default()
    }

    fn worktree_id(&self, _cx: &App) -> project::WorktreeId {
        // HACK: should we use the original diff file's worktree here?
        project::WorktreeId::from_usize(0)
    }

    fn to_proto(&self, _cx: &App) -> proto::File {
        todo!()
    }

    fn is_private(&self) -> bool {
        false
    }

    fn can_open(&self) -> bool {
        false
    }
}
