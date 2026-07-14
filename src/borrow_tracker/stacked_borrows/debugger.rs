use std::ops::Range;

use ratatui::prelude::*;
use ratatui::widgets::*;
use rustc_abi::{Align, Size};
use rustc_const_eval::interpret::AllocInfo;

use crate::*;

#[derive(Clone, Debug)]
pub struct DebuggerBorrowStacks {
    pub whole: DebuggerWholeAllocation,
    pub segments: Vec<DebuggerSegment>,
}

impl DebuggerBorrowStacks {
    pub fn new() -> Self {
        DebuggerBorrowStacks {
            whole: DebuggerWholeAllocation {
                // Safety: AllocId is a mere usize with PhantomData.
                alloc_id: AllocId(1.try_into().unwrap()),
                info: AllocInfo {
                    size: Size::ZERO,
                    align: Align::ONE,
                    kind: AllocKind::Dead,
                    mutbl: rustc_ast::Mutability::Not,
                },
            },
            segments: vec![],
        }
    }

    pub fn header() -> (Row<'static>, [Constraint; 9]) {
        let row = Row::new(vec![
            cell_left("AllocId"),
            cell_right("Bytes"),
            cell_right("Align"),
            cell_right("PosStart"),
            cell_right("PosEnd"),
            cell_right("BorTagID"),
            cell_right("StackIdx"),
            cell_right("Permission"),
            cell_right("Protected"),
        ]);
        let widths = [
            Constraint::Min(8),  // AllocId
            Constraint::Min(4),  // Bytes
            Constraint::Min(4),  // Align
            Constraint::Min(8),  // PosStart
            Constraint::Min(8),  // PosEnd
            Constraint::Min(10), // BorTagID
            Constraint::Min(8),  // StackIdx
            Constraint::Min(15), // Permission
            Constraint::Min(4),  // Protected
        ];
        (row, widths)
    }

    pub fn to_table_rows(&self) -> Vec<Row<'static>> {
        let mut level1 = true;
        let mut level2 = true;
        let mut rows = Vec::with_capacity(128);

        for seg in &self.segments {
            let whole = &self.whole;
            let Range { start, end } = seg.range;

            for (idx, item) in seg.stack.iter().rev().enumerate() {
                let idx = Text::from(idx.to_string()).style(Color::DarkGray);
                let permission = match item.permission {
                    Permission::Unique => Text::from("Unique").style(Color::Blue),
                    Permission::SharedReadWrite => Text::from("SharedReadWrite").style(Color::Cyan),
                    Permission::SharedReadOnly =>
                        Text::from("SharedReadOnly").style(Color::LightMagenta),
                    Permission::Disabled => Text::from("Disabled").style(Color::Red),
                };
                let row = Row::new(vec![
                    if level1 { cell_left(whole.alloc_id.0) } else { empty_cell() },
                    level_cell(level1, hsize(whole.info.size.bytes())),
                    level_cell(level1, hsize(whole.info.align.bytes())),
                    level_cell(level2, start),
                    level_cell(level2, end),
                    cell_right(item.bor_tag_id),
                    Cell::new(idx.right_aligned()),
                    Cell::new(permission.right_aligned()),
                    cell_right(if item.protected { "true" } else { "" }),
                ]);
                rows.push(row);

                level1 = false;
                level2 = false;
            }
            level2 = true;
        }

        rows
    }
}

fn cell_right(val: impl ToString) -> Cell<'static> {
    Cell::new(Text::from(val.to_string()).right_aligned())
}

fn cell_left(val: impl ToString) -> Cell<'static> {
    Cell::new(Text::from(val.to_string()))
}

fn empty_cell() -> Cell<'static> {
    Cell::default()
}

fn level_cell(level: bool, val: impl ToString) -> Cell<'static> {
    if level { cell_right(val.to_string()) } else { empty_cell() }
}

#[derive(Clone, Debug)]
pub struct DebuggerWholeAllocation {
    pub alloc_id: AllocId,
    pub info: AllocInfo,
}

#[derive(Clone, Debug)]
pub struct DebuggerSegment {
    pub range: Range<u64>,
    pub stack: Vec<DebuggerBorrowStackItem>,
}

#[derive(Clone, Debug)]
pub struct DebuggerBorrowStackItem {
    pub bor_tag_id: u64,
    pub permission: Permission,
    pub protected: bool,
}
