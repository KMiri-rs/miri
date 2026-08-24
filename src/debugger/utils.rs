use std::sync::{LazyLock, Mutex};

use ratatui::text::{Line, Span as RatatuiSpan};
use rustc_data_structures::fx::FxHashMap;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;
use rustc_span::source_map::SourceMap;
use rustc_span::{FileName, RealFileName, Span};

use crate::concurrency::thread::EvalContextExt;
use crate::debugger::state::RenderSrc;
use crate::debugger::tui::theme::STYLE_HIGHTLIGHTED;
use crate::helpers::ToUsize;
use crate::{MemoryKind, MiriInterpCx, MiriMemoryKind};

pub fn kind_str(kind: MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Stack => "Stack",
        MemoryKind::CallerLocation => "CallerLoc",
        MemoryKind::Machine(kind) =>
            match kind {
                MiriMemoryKind::Kernel => "Kernel",
                MiriMemoryKind::Rust => "Rust",
                MiriMemoryKind::Miri => "Miri",
                MiriMemoryKind::C => "C",
                MiriMemoryKind::WinHeap => "WinHeap",
                MiriMemoryKind::WinLocal => "WinLocal",
                MiriMemoryKind::Machine => "Machine",
                MiriMemoryKind::Runtime => "Runtime",
                MiriMemoryKind::Global => "Global",
                MiriMemoryKind::ExternStatic => "ExternStatic",
                MiriMemoryKind::Tls => "Tls",
                MiriMemoryKind::Mmap => "Mmap",
                MiriMemoryKind::SocketAddress => "SocketAddress",
            },
    }
}

pub fn hsize(n: impl humansize::ToF64 + humansize::Unsigned) -> String {
    humansize::format_size(n, humansize::BINARY)
}

#[expect(unused)]
pub fn find_def_id_from_span(tcx: TyCtxt<'_>, span: Span) -> Option<(Span, DefId)> {
    let mut best: Option<(Span, DefId)> = None;

    for local_def_id in tcx.hir_crate_items(()).definitions() {
        let def_id = local_def_id.to_def_id();
        let Some(def_span) = tcx.hir_span_if_local(def_id) else {
            continue;
        };

        if def_span.contains(span) {
            match best {
                None => best = Some((def_span, def_id)),
                Some((best_span, _)) => {
                    // 选更“内层”的那个定义
                    if best_span.contains(def_span) && !def_span.contains(best_span) {
                        best = Some((def_span, def_id));
                    }
                }
            }
        }
    }

    best
}

pub fn source_file(sm: &SourceMap, span: Span) -> String {
    // Force path remapping, because `prefer_remapped_unconditionally` doesn't always work.
    // Use `--remap-path-prefix` to shorten the long sysroot path, e.g.
    // ./miri run tests/pass/debugger_test.rs --debugger --remap-path-prefix=$(rustc --print=sysroot)/lib/rustlib/src/rust/library/=
    match sm.span_to_filename(span) {
        FileName::Real(path) if let Some(local_path) = path.clone().into_local_path() =>
            FileName::Real(sm.path_mapping().to_real_filename(&RealFileName::empty(), local_path)),
        file_name => file_name,
    }
    .prefer_remapped_unconditionally()
    .to_string()
}

pub fn render_src(
    body_span: rustc_span::Span,
    highlight_span: rustc_span::Span,
    sm: &SourceMap,
) -> RenderSrc {
    let source_text = match sm.span_to_snippet(body_span) {
        Ok(source_text) => source_text,
        Err(err) =>
            return RenderSrc {
                lines: vec![
                    "Could not load source snippet:".into(),
                    format!("{err:?}").into(),
                    "body_span".into(),
                    format!("  ={body_span:?}").into(),
                ],
                highlighted_idx: None,
            },
    };

    // Get the absolute byte positions for relative calculations
    let body_lo = body_span.lo();
    let highlight_lo = highlight_span.lo();
    let highlight_hi = highlight_span.hi();

    let mut render_src = RenderSrc::default();
    let lines = &mut render_src.lines;

    let mut current_pos = body_lo;

    // Split the source by lines and construct Ratatui Line/Span structures
    for (idx, text_line) in source_text.lines().enumerate() {
        let line_len = text_line.len().try_into().unwrap();
        let line_end = current_pos + rustc_span::BytePos(line_len);

        let mut line_spans = Vec::new();

        // Check if the current line intersects with the highlight_span
        // Case A: The line is entirely before the highlight
        // Case B: The line is entirely after the highlight
        if line_end <= highlight_lo || current_pos >= highlight_hi {
            // No intersection: add as plain text
            line_spans.push(RatatuiSpan::raw(text_line.to_string()));
        } else {
            // Intersection exists: split the line into parts
            let line_start_pos = current_pos;

            // Calculate relative start and end indices for the highlight within this specific line
            let h_start_in_line = if highlight_lo > line_start_pos {
                highlight_lo.0.checked_sub(line_start_pos.0).unwrap().to_usize()
            } else {
                0
            };

            let h_end_in_line = if highlight_hi < line_end {
                highlight_hi.0.checked_sub(line_start_pos.0).unwrap().to_usize()
            } else {
                text_line.len()
            };

            // Part 1: Text before the highlight
            if h_start_in_line > 0 {
                line_spans.push(RatatuiSpan::raw(text_line[..h_start_in_line].to_string()));
            }

            // Part 2: The highlighted text (Styled with BOLD and UNDERLINE)
            line_spans.push(RatatuiSpan::styled(
                text_line[h_start_in_line..h_end_in_line].to_string(),
                STYLE_HIGHTLIGHTED,
            ));

            // Part 3: Text after the highlight
            if h_end_in_line < text_line.len() {
                line_spans.push(RatatuiSpan::raw(text_line[h_end_in_line..].to_string()));
            }

            // Update highlighted_idx
            let idx = u16::try_from(idx).unwrap();
            render_src.highlighted_idx = Some(match render_src.highlighted_idx {
                Some([start, end]) => [start.min(idx), end.max(idx)],
                None => [idx; 2],
            });
        }

        lines.push(Line::from(line_spans));

        // Update current position for the next iteration (+1 to account for the '\n' character)
        current_pos = line_end + rustc_span::BytePos(1);
    }
    render_src
}

#[derive(Default, Debug, Clone, Copy)]
pub struct InverseIdx {
    pub alloc: usize,
    pub bs_segment: usize,
    pub bs_stack: usize,
}

/// Given the highlighted_idx and area height, center the view by returning the start row idx.
pub fn src_view_centering(highlighted_idx: [u16; 2], height: u16) -> u16 {
    let [start_idx, end_idx] = highlighted_idx;
    let [start_idx, end_idx]: [usize; 2] = [start_idx.into(), end_idx.into()];

    let height: usize = height.into();
    let scroll = if end_idx + 2 < height {
        // The highlighted lines fit into current view from first line.
        0
    } else {
        // Pan the view to the first highlighted line.
        let gap = height.checked_sub(1 + end_idx - start_idx).unwrap_or(height) / 2;
        start_idx.saturating_sub(gap)
    };
    scroll.try_into().unwrap()
}

pub fn pos_to_line_nr(sm: &SourceMap, pos: rustc_span::BytePos) -> u16 {
    let loc = sm.lookup_char_pos(pos);
    u16::try_from(loc.line).unwrap_or(0)
}

pub fn instance_name(ecx: &MiriInterpCx<'_>, def_id: DefId) -> String {
    use rustc_middle::ty::print::{with_no_trimmed_paths, with_resolve_crate_name};

    static RECORDED: LazyLock<Mutex<FxHashMap<DefId, String>>> = LazyLock::new(Default::default);

    let mut recorded = RECORDED.lock().unwrap();
    recorded
        .entry(def_id)
        .or_insert_with(|| {
            with_no_trimmed_paths!(with_resolve_crate_name!(ecx.tcx.def_path_str(def_id)))
        })
        .clone()
}

/// Checks if the current frame matches `src_line` (e.g., "path/to/file.rs:10" or "path/to/file.rs").
///
/// Matches the name part against `frame.fn_name` and the optional line number against `frame.line_start`.
pub fn is_src_line_reached(ecx: &MiriInterpCx<'_>, src_line: &str) -> bool {
    let Some(frame) = ecx.active_thread_stack().last() else {
        return true;
    };

    let sm = ecx.tcx.sess.source_map();
    let span = frame.current_span();
    let file_name = source_file(sm, span);
    let line_start = pos_to_line_nr(sm, span.lo());

    let Some((file, line)) = src_line.rsplit_once(':') else {
        return src_line == file_name;
    };
    if file != file_name {
        return false;
    }
    line.parse::<u16>().map(|line| line == line_start).unwrap_or(false)
}
