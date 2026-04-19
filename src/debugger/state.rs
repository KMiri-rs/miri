use ratatui::style::{Color, Modifier, Style, Styled};
use ratatui::text::{Line, Span as RatatuiSpan};
use rustc_data_structures::either::Either;
use rustc_middle::mir::{self, BasicBlockData};
use rustc_span::source_map::SourceMap;

use crate::debugger::tui::theme::STYLE_HIGHTLIGHTED;
use crate::*;

#[derive(Clone, Debug)]
pub struct FrameInfo {
    pub fn_name: String,
    pub source_file: String,
    pub line_start: u32,
    pub line_end: u32,
    pub locals: Vec<LocalInfo>,
}

#[derive(Clone, Debug)]
pub struct CurrentLocation {
    /// Full source code with current location highlighted.
    pub render_src: Vec<Line<'static>>,
    pub line_start: u32,
    pub line_end: u32,
    /// A basic block with current location highlighted.
    pub render_mir: Vec<String>,
    pub render_mir_highlighted_idx: u32,
}

#[derive(Clone, Debug)]
pub struct LocalInfo {
    pub idx: String,
    pub name: String,
    pub value: String,
    pub ty: String,
    pub state: LocalKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    Initialized,
    Pointer,
    Uninitialized,
    Dead,
}

#[derive(Clone, Debug)]
pub struct CfgLine {
    pub block: usize,
    pub text: String,
    pub is_current: bool,
    pub successors: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct MemoryInfo {
    pub name: String,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct OutputLine {
    pub is_stderr: bool,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct DebuggerState {
    pub current_thread: ThreadId,
    pub step_count: u64,
    pub in_user_code: bool,
    pub stack_frames: Vec<FrameInfo>,
    pub current_location: CurrentLocation,
    pub cfg_lines: Vec<CfgLine>,
    pub locals: Vec<LocalInfo>,
    pub memory: Vec<MemoryInfo>,
    pub output: Vec<OutputLine>,
}

impl DebuggerState {
    pub fn capture<'tcx>(ecx: &MiriInterpCx<'tcx>) -> Self {
        let sm = ecx.tcx.sess.source_map();
        let stack = ecx.active_thread_stack();

        let stack_frames: Vec<_> =
            stack.iter().rev().map(|frame| capture_frame(sm, frame)).collect();

        let current_location =
            stack.last().map(|frame| capture_location(ecx, frame)).unwrap_or_else(|| {
                CurrentLocation {
                    render_src: vec!["No stack frame found.".into()],
                    line_start: 0,
                    line_end: 0,
                    render_mir: vec!["No basic block found.".into()],
                    render_mir_highlighted_idx: 0,
                }
            });

        let locals = stack.last().map(capture_locals).unwrap_or_default();
        let in_user_code =
            stack_frames.last().map(|frame| is_user_code_path(&frame.source_file)).unwrap_or(true);
        let cfg_lines = stack.last().map(capture_cfg_lines).unwrap_or_default();
        let memory = capture_memory(ecx, &locals);
        let output = ecx
            .machine
            .debugger_output
            .borrow()
            .iter()
            .map(|(is_stderr, text)| OutputLine { is_stderr: *is_stderr, text: text.clone() })
            .collect();

        Self {
            current_thread: ecx.active_thread(),
            step_count: ecx.machine.basic_block_count,
            in_user_code,
            stack_frames,
            current_location,
            cfg_lines,
            locals,
            memory,
            output,
        }
    }
}

fn is_user_code_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.contains(".rustup\\toolchains\\miri") || lower.contains(".rustup/toolchains/miri") {
        return false;
    }
    if path.starts_with('<') {
        return false;
    }
    true
}

fn capture_locals(frame: &Frame<'_, Provenance, FrameExtra<'_>>) -> Vec<LocalInfo> {
    frame
        .locals
        .iter()
        .enumerate()
        .map(|(idx, local)| {
            let local_idx = mir::Local::from_usize(idx);
            let body = frame.body();
            let local_decl = &body.local_decls[local_idx];
            let raw = format!("{local:?}");
            let (value, kind) = prettify_local_value(&raw, &local_decl.ty.to_string());
            LocalInfo {
                idx: format!("_{idx}"),
                name: find_name_for_local(body, local_idx)
                    .map(|x| x.to_string())
                    .unwrap_or_default(),
                value,
                ty: local_decl.ty.to_string(),
                state: kind,
            }
        })
        .collect()
}

pub fn find_name_for_local(body: &mir::Body<'_>, local: mir::Local) -> Option<rustc_span::Symbol> {
    body.var_debug_info.iter().find_map(|var_info| {
        if let mir::VarDebugInfoContents::Place(place) = var_info.value {
            if place.local == local && place.projection.as_slice().is_empty() {
                return Some(var_info.name);
            }
        }
        None
    })
}

fn pos_to_line_nr(sm: &SourceMap, pos: rustc_span::BytePos) -> u32 {
    let loc = sm.lookup_char_pos(pos);
    u32::try_from(loc.line).unwrap_or(0)
}

fn capture_frame(sm: &SourceMap, frame: &Frame<'_, Provenance, FrameExtra<'_>>) -> FrameInfo {
    let span = frame.current_span();
    FrameInfo {
        fn_name: frame.instance().to_string(),
        source_file: {
            // Force path remapping, because `prefer_remapped_unconditionally` doesn't always work.
            // Use `--remap-path-prefix` to shorten the long sysroot path, e.g.
            // ./miri run tests/pass/debugger_test.rs --debugger --remap-path-prefix=$(rustc --print=sysroot)/lib/rustlib/src/rust/library/=
            let file_name =
                sm.span_to_filename(span).into_local_path().unwrap_or_else(|| "Unknown".into());
            let (path, _) = sm.path_mapping().map_prefix(file_name);
            path.display().to_string()
        },
        line_start: pos_to_line_nr(sm, span.lo()),
        line_end: pos_to_line_nr(sm, span.hi()),
        locals: capture_locals(frame),
    }
}

fn capture_cfg_lines(frame: &Frame<'_, Provenance, FrameExtra<'_>>) -> Vec<CfgLine> {
    let current_block = match frame.current_loc() {
        Either::Left(loc) => Some(loc.block.index()),
        Either::Right(_) => None,
    };

    frame
        .body()
        .basic_blocks
        .iter_enumerated()
        .map(|(bb, block_data)| {
            let bb_idx = bb.index();
            let mut text = format!("bb{bb_idx}");
            let mut successors = Vec::new();
            if let Some(term) = &block_data.terminator {
                let succs: Vec<usize> = term.successors().map(|target| target.index()).collect();
                successors = succs.clone();
                if succs.is_empty() {
                    text.push_str(" -> <end>");
                } else {
                    text.push_str(" -> ");
                    text.push_str(
                        &succs.iter().map(|s| format!("bb{s}")).collect::<Vec<_>>().join(", "),
                    );
                }
            }
            CfgLine { block: bb_idx, text, is_current: current_block == Some(bb_idx), successors }
        })
        .collect()
}

fn capture_memory(ecx: &MiriInterpCx<'_>, locals: &[LocalInfo]) -> Vec<MemoryInfo> {
    let mut entries = Vec::new();

    let alloc_spans = ecx.machine.allocation_spans.borrow();
    entries.push(MemoryInfo {
        name: "allocations".to_string(),
        detail: alloc_spans.len().to_string(),
    });

    for (alloc_id, (_alloc, dealloc)) in alloc_spans.iter().take(32) {
        entries.push(MemoryInfo {
            name: format!("{alloc_id:?}"),
            detail: if dealloc.is_some() { "deallocated" } else { "live" }.to_string(),
        });
    }

    for local in locals.iter().filter(|l| l.state == LocalKind::Pointer).take(16) {
        entries
            .push(MemoryInfo { name: format!("ptr {}", local.idx), detail: local.value.clone() });
    }

    entries
}

fn prettify_local_value(raw: &str, ty: &str) -> (String, LocalKind) {
    let lower = raw.to_ascii_lowercase();

    if lower.contains("dead") {
        return ("-".to_string(), LocalKind::Dead);
    }
    if lower.contains("uninit") {
        return ("uninit".to_string(), LocalKind::Uninitialized);
    }

    if let Some(hex) = extract_hex_scalar(raw) {
        if is_pointer_type(ty) {
            if hex == 0 {
                return ("null".to_string(), LocalKind::Pointer);
            }
            return (format!("ptr(0x{hex:x})"), LocalKind::Pointer);
        }
        if ty == "bool" {
            return ((hex != 0).to_string(), LocalKind::Initialized);
        }
        if let Ok(num) = i128::try_from(hex) {
            return (num.to_string(), LocalKind::Initialized);
        }
        return (format!("0x{hex:x}"), LocalKind::Initialized);
    }

    if is_pointer_type(ty) {
        return (compact_debug(raw), LocalKind::Pointer);
    }

    (compact_debug(raw), LocalKind::Initialized)
}

fn extract_hex_scalar(raw: &str) -> Option<u128> {
    let scalar_pos = raw.find("Scalar(")?;
    let tail = &raw[scalar_pos..];
    let start = tail.find("0x")? + scalar_pos;
    let rest = &raw[start + 2..];
    let hex_len = rest.chars().take_while(|c| c.is_ascii_hexdigit()).count();
    if hex_len == 0 {
        return None;
    }
    u128::from_str_radix(&rest[..hex_len], 16).ok()
}

fn is_pointer_type(ty: &str) -> bool {
    ty.contains('*') || ty.contains('&')
}

fn compact_debug(raw: &str) -> String {
    raw.replace("LocalState { value: Live(", "")
        .replace("), ty: No }", "")
        .replace("Immediate(", "")
        .replace("Scalar(", "")
        .trim()
        .to_string()
}

/// Renders the source code of a MIR Body and highlights the source range
/// corresponding to a specific MIR Location using Ratatui styles.
fn capture_location(
    ecx: &MiriInterpCx<'_>,
    frame: &Frame<'_, Provenance, FrameExtra<'_>>,
) -> CurrentLocation {
    let sm = ecx.tcx.sess.source_map();
    let body = frame.body();
    let current_loc = match frame.current_loc() {
        Either::Left(loc) => loc,
        Either::Right(span) => todo!(),
    };

    // 1. Get the source range of the entire MIR Body
    let body_span = body.span;

    // 2. Get the Span corresponding to the current MIR Location (Statement or Terminator)
    let highlight_span =
        if current_loc.statement_index < body.basic_blocks[current_loc.block].statements.len() {
            body.basic_blocks[current_loc.block].statements[current_loc.statement_index]
                .source_info
                .span
        } else {
            body.basic_blocks[current_loc.block].terminator().source_info.span
        };

    // Use source_callsite() to resolve the span to the actual physical file
    // instead of inside a macro expansion if possible.
    let body_span = body_span.source_callsite();
    let highlight_span = highlight_span.source_callsite();

    let line_start = pos_to_line_nr(sm, highlight_span.lo());
    let line_end = pos_to_line_nr(sm, highlight_span.hi());

    // 3. Extract the raw source text snippet
    let Ok(source_text) = sm.span_to_snippet(body_span) else {
        return CurrentLocation {
            render_src: vec!["Could not load source snippet.".into()],
            line_start,
            line_end,
            render_mir: vec!["Could not load basic block.".into()],
            render_mir_highlighted_idx: 0,
        };
    };

    // Get the absolute byte positions for relative calculations
    let body_lo = body_span.lo();
    let highlight_lo = highlight_span.lo();
    let highlight_hi = highlight_span.hi();

    let mut lines = Vec::new();

    let mut current_pos = body_lo;

    // Split the source by lines and construct Ratatui Line/Span structures
    for text_line in source_text.lines() {
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
        }

        lines.push(Line::from(line_spans));

        // Update current position for the next iteration (+1 to account for the '\n' character)
        current_pos = line_end + rustc_span::BytePos(1);
    }

    let bb = body.basic_blocks.get(current_loc.block).unwrap();
    let (render_mir, render_mir_highlighted_idx) = current_mir(bb, current_loc.statement_index);

    CurrentLocation {
        render_src: lines,
        line_start,
        line_end,
        render_mir,
        render_mir_highlighted_idx,
    }
}

fn current_mir(bb: &BasicBlockData<'_>, stmt_idx: usize) -> (Vec<String>, u32) {
    let stmt_len = bb.statements.len();
    let hightlighted = if stmt_idx < stmt_len { stmt_idx } else { stmt_len };

    let mut v = Vec::with_capacity(stmt_len + 1);
    v.extend(bb.statements.iter().map(|stmt| format!("{:?}", stmt.kind)));
    v.push(format!("{:?}", bb.terminator().kind));
    (v, hightlighted.try_into().unwrap())
}
