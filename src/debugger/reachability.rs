#![allow(rustc::internal)]
use std::collections::VecDeque;

use rustc_data_structures::fx::FxHashSet;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::visit::Visitor as _;
use rustc_middle::mir::{self, HasLocalDecls};
use rustc_middle::ty::{self, Instance, InstanceKind, Ty, TyCtxt, TypeVisitableExt};
use rustc_public::mir::MirVisitor;
use rustc_public::mir::visit::Location;
use rustc_public::ty::RigidTy;
use rustc_span::source_map::SourceMap;

use crate::debugger::utils::{pos_to_line_nr, source_file};

extern crate rustc_public;

#[derive(Clone, Debug)]
pub struct FunctionInstanceInfo {
    pub instance: String,
    pub source_file: String,
    pub line_start: u16,
    pub line_end: u16,
}

struct CollectInstance<'tcx> {
    v_instance: Vec<Instance<'tcx>>,
    tcx: TyCtxt<'tcx>,
}

impl MirVisitor for CollectInstance<'_> {
    fn visit_ty(&mut self, ty: &rustc_public::ty::Ty, location: Location) {
        if let rustc_public::ty::TyKind::RigidTy(RigidTy::FnDef(fn_def, args)) = ty.kind() {
            if let Ok(instance) = rustc_public::mir::mono::Instance::resolve(fn_def, &args) {
                log!("visit: {:?}", instance.name());
                self.v_instance.push(rustc_public::rustc_internal::internal(self.tcx, instance));
            }
        }
        self.super_ty(ty);
    }
}

pub fn collect<'tcx>(tcx: TyCtxt<'tcx>) -> Box<[FunctionInstanceInfo]> {
    let mut collector = CollectInstance { v_instance: Vec::with_capacity(1024), tcx };

    let local_fn_defs = rustc_public::local_crate().fn_defs().into_iter();
    let dep_fn_defs = rustc_public::external_crates()
        .into_iter()
        .filter(|krate| {
            let crate_num = rustc_public::rustc_internal::internal(tcx, krate.id);
            for path in tcx.crate_extern_paths(crate_num) {
                if let Ok(path) = path.canonicalize() {
                    if path.starts_with("/home/zjp/KMiri/asterinas/") {
                        return true;
                    }
                }
            }
            false
        })
        .flat_map(|krate| krate.fn_defs());
    for fn_def in local_fn_defs.chain(dep_fn_defs) {
        if let Some(body) = fn_def.body() {
            collector.visit_body(&body);
        }
    }

    let sm = tcx.sess.source_map();
    let mut v_fn: Box<[_]> = collector
        .v_instance
        .into_iter()
        .map(|instance| {
            let def_id = instance.def_id();
            let span = tcx.def_span(def_id);
            FunctionInstanceInfo {
                instance: instance.to_string(),
                source_file: source_file(sm, span),
                line_start: pos_to_line_nr(sm, span.lo()),
                line_end: pos_to_line_nr(sm, span.hi()),
            }
        })
        .collect();
    v_fn.sort_unstable_by(|a, b| a.instance.cmp(&b.instance));

    log!("collect_and_partition_mono_items: {v_fn:#?}");
    v_fn
}

pub fn collect_reachable_function_instances<'tcx>(
    tcx: TyCtxt<'tcx>,
    entry_id: DefId,
    sm: &SourceMap,
) -> Vec<FunctionInstanceInfo> {
    collect(tcx);

    let mut pending = VecDeque::new();
    let mut seen = FxHashSet::default();
    let mut reachable = Vec::new();

    push_instance(Instance::mono(tcx, entry_id), &mut pending, &mut seen);

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
    instance: Instance<'tcx>,
    pending: &mut VecDeque<Instance<'tcx>>,
    seen: &mut FxHashSet<Instance<'tcx>>,
) {
    if instance.args.has_non_region_param() {
        return;
    }
    if matches!(instance.def, InstanceKind::Intrinsic(_) | InstanceKind::Virtual(..)) {
        return;
    }
    if seen.insert(instance) {
        pending.push_back(instance);
    }
}

fn collect_from_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    pending: &mut VecDeque<Instance<'tcx>>,
    seen: &mut FxHashSet<Instance<'tcx>>,
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
    tcx: TyCtxt<'tcx>,
    alloc_id: rustc_middle::mir::interpret::AllocId,
    pending: &mut VecDeque<Instance<'tcx>>,
    seen: &mut FxHashSet<Instance<'tcx>>,
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
    tcx: TyCtxt<'tcx>,
    body: &'a mir::Body<'tcx>,
    pending: &'a mut VecDeque<Instance<'tcx>>,
    seen: &'a mut FxHashSet<Instance<'tcx>>,
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
