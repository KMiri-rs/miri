use std::collections::VecDeque;

use ratatui::text::{Line, Span as RatatuiSpan};
use rustc_data_structures::either::Either;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_hir::def_id::DefId;
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::visit::Visitor as _;
use rustc_middle::mir::{self, BasicBlockData, HasLocalDecls};
use rustc_middle::ty::{self, TypeVisitableExt};
use rustc_span::source_map::SourceMap;

use crate::debugger::utils::{pos_to_line_nr, source_file};

#[derive(Clone, Debug)]
pub struct FunctionInstanceInfo {
    pub instance: String,
    pub source_file: String,
    pub line_start: u16,
    pub line_end: u16,
}

pub fn collect_reachable_function_instances<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    entry_id: DefId,
    sm: &SourceMap,
) -> Vec<FunctionInstanceInfo> {
    let mut pending = VecDeque::new();
    let mut seen = FxHashSet::default();
    let mut reachable = Vec::new();

    push_instance(ty::Instance::mono(tcx, entry_id), &mut pending, &mut seen);

    while let Some(instance) = pending.pop_front() {
        let span = tcx.def_span(instance.def_id());
        reachable.push(FunctionInstanceInfo {
            instance: instance.to_string(),
            source_file: source_file(sm, span),
            line_start: pos_to_line_nr(sm, span.lo()),
            line_end: pos_to_line_nr(sm, span.hi()),
        });

        if tcx.is_foreign_item(instance.def_id()) {
            continue;
        }

        let body = tcx.instance_mir(instance.def);

        let mut visitor = ReachabilityVisitor { tcx, body, pending: &mut pending, seen: &mut seen };
        visitor.visit_body(body);
    }

    reachable
}

fn push_instance<'tcx>(
    instance: ty::Instance<'tcx>,
    pending: &mut VecDeque<ty::Instance<'tcx>>,
    seen: &mut FxHashSet<ty::Instance<'tcx>>,
) {
    if instance.args.has_non_region_param() {
        return;
    }
    if matches!(instance.def, ty::InstanceKind::Intrinsic(_) | ty::InstanceKind::Virtual(..)) {
        return;
    }
    if seen.insert(instance) {
        pending.push_back(instance);
    }
}

fn collect_from_ty<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    pending: &mut VecDeque<ty::Instance<'tcx>>,
    seen: &mut FxHashSet<ty::Instance<'tcx>>,
) {
    if ty.has_non_region_param() {
        return;
    }
    if let ty::TyKind::FnDef(def, args) = ty.kind() {
        if let Ok(Some(instance)) = ty::Instance::try_resolve(
            tcx,
            ty::TypingEnv::fully_monomorphized(),
            *def,
            args.skip_binder(),
        ) {
            push_instance(instance, pending, seen);
        }
    }
}

fn collect_from_alloc<'tcx>(
    tcx: ty::TyCtxt<'tcx>,
    alloc_id: rustc_middle::mir::interpret::AllocId,
    pending: &mut VecDeque<ty::Instance<'tcx>>,
    seen: &mut FxHashSet<ty::Instance<'tcx>>,
) {
    let Some(GlobalAlloc::Memory(alloc)) = tcx.try_get_global_alloc(alloc_id) else {
        return;
    };

    for (_, prov) in alloc.0.provenance().ptrs().iter() {
        if let GlobalAlloc::Function { instance } = tcx.global_alloc(prov.alloc_id()) {
            push_instance(instance, pending, seen);
        }
    }
}

struct ReachabilityVisitor<'a, 'tcx> {
    tcx: ty::TyCtxt<'tcx>,
    body: &'a mir::Body<'tcx>,
    pending: &'a mut VecDeque<ty::Instance<'tcx>>,
    seen: &'a mut FxHashSet<ty::Instance<'tcx>>,
}

impl<'tcx> mir::visit::Visitor<'tcx> for ReachabilityVisitor<'_, 'tcx> {
    fn visit_operand(&mut self, operand: &mir::Operand<'tcx>, location: mir::Location) {
        match operand {
            mir::Operand::Copy(place) | mir::Operand::Move(place) => {
                collect_from_ty(
                    self.tcx,
                    place.ty(self.body.local_decls(), self.tcx).ty,
                    self.pending,
                    self.seen,
                );
            }
            mir::Operand::Constant(_) => {}
            mir::Operand::RuntimeChecks(_) => {}
        }

        self.super_operand(operand, location);
    }

    fn visit_const_operand(&mut self, constant: &mir::ConstOperand<'tcx>, location: mir::Location) {
        if let mir::Const::Val(val, ty) = &constant.const_ {
            if ty.is_fn_ptr() {
                if let Some(scalar) = val.try_to_scalar()
                    && let Some(ptr) = scalar.to_pointer(&self.tcx).discard_err()
                {
                    let Some(prov) = ptr.provenance else {
                        self.super_const_operand(constant, location);
                        return;
                    };
                    if let GlobalAlloc::Function { instance } =
                        self.tcx.global_alloc(prov.alloc_id())
                    {
                        push_instance(instance, self.pending, self.seen);
                    }
                }
            } else if let mir::ConstValue::Indirect { alloc_id, .. } = val {
                collect_from_alloc(self.tcx, *alloc_id, self.pending, self.seen);
            }
        }

        let ty = constant.const_.ty();
        if !ty.has_non_region_param() {
            collect_from_ty(self.tcx, ty, self.pending, self.seen);
        }
        self.super_const_operand(constant, location);
    }
}
