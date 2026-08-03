// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

// 'compilation is the lifetime of compiler interface object supplied to the after_analysis call back.
// 'tcx is the lifetime of the type context created during the lifetime of the after_analysis call back.
// 'analysis is the life time of the analyze_with_mirai call back that is invoked with the type context.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::rc::Rc;
use std::time::Instant;

use log::*;
use log_derive::{logfn, logfn_inputs};

use mirai_annotations::*;
use rustc_errors::Diag;
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_middle::mir;
use rustc_middle::ty::{GenericArgsRef, TyCtxt};
use rustc_session::Session;

use crate::body_visitor::BodyVisitor;
use crate::call_graph::CallGraph;
use crate::constant_domain::ConstantValueCache;
use crate::coverage::{
    count_gaps, envelope_is_sealed, CoverageGapKind, CoverageManifest, CoverageOutcome,
    CoverageRecord,
};
use crate::expected_errors;
use crate::known_names::KnownNamesCache;
use crate::options::Options;
use crate::summaries::SummaryCache;
use crate::tag_domain::Tag;
use crate::type_visitor::TypeCache;
use crate::utils;

/// A visitor that takes information gathered by the Rust compiler when compiling a particular
/// crate and then analyses some of the functions in that crate to see if any of the assertions
/// and implicit assertions in the MIR bodies might be false and generates warning for those.
///
// 'compilation is the lifetime of the call to MiraiCallbacks::after_analysis.
// 'tcx is the lifetime of the closure call that calls analyze_with_mirai, which calls analyze_some_bodies.
pub struct CrateVisitor<'compilation, 'tcx> {
    pub buffered_diagnostics: Vec<Diag<'compilation, ()>>,
    pub constant_time_tag_cache: Option<Tag>,
    pub constant_time_tag_not_found: bool,
    pub constant_value_cache: ConstantValueCache<'tcx>,
    pub coverage_for: HashMap<DefId, Vec<CoverageRecord>>,
    pub diagnostics_for: HashMap<DefId, Vec<Diag<'compilation, ()>>>,
    pub file_name: &'compilation str,
    pub generic_args_cache: HashMap<DefId, GenericArgsRef<'tcx>>,
    pub known_names_cache: KnownNamesCache,
    pub options: &'compilation Options,
    pub region_gaps: Vec<CoverageRecord>,
    pub session: &'compilation Session,
    pub summary_cache: SummaryCache<'tcx>,
    pub tcx: TyCtxt<'tcx>,
    pub type_cache: Rc<RefCell<TypeCache<'tcx>>>,
    pub test_run: bool,
    pub call_graph: CallGraph<'tcx>,
}

impl Debug for CrateVisitor<'_, '_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        "CrateVisitor".fmt(f)
    }
}

impl<'compilation> CrateVisitor<'compilation, '_> {
    /// Analyze some of the bodies in the crate that is being compiled.
    #[logfn(TRACE)]
    pub fn analyze_some_bodies(&mut self) -> bool {
        let start_instant = Instant::now();
        // Determine the functions we want to analyze.
        let selected_functions = self.get_selected_function_list();

        // Get the entry function
        let entry_fn_def_id = if let Some((def_id, _)) = self.tcx.entry_fn(()) {
            def_id
        } else {
            DefId::local(DefIndex::from_u32(0))
        };

        // Analyze all functions that are whitelisted or public
        let building_standard_summaries = std::env::var("MIRAI_START_FRESH").is_ok();
        let body_owners = self.tcx.hir_body_owners().collect::<Vec<_>>();
        let mut accounted_roots = 0;
        let mut completed_enumeration = true;
        for (body_index, local_def_id) in body_owners.iter().copied().enumerate() {
            let def_id = local_def_id.to_def_id();
            let name = utils::summary_key_str(self.tcx, def_id);
            if let Some(selections) = &selected_functions {
                if !self.included_in(selections.as_ref(), name.as_ref(), def_id) {
                    if self.options.single_func.is_none() {
                        debug!(
                            "skipping function {} as it is not selected for analysis",
                            name
                        );
                    }
                    self.record_region_gap(def_id, CoverageGapKind::SkippedRoot);
                    accounted_roots += 1;
                    continue;
                }
                info!("analyzing selected function {}", name);
            } else if !building_standard_summaries {
                if !utils::is_public(def_id, self.tcx) && def_id != entry_fn_def_id {
                    debug!("skipping function {} as it is not public", name);
                    self.record_region_gap(def_id, CoverageGapKind::SkippedRoot);
                    accounted_roots += 1;
                    continue;
                } else if self
                    .tcx
                    .generics_of(def_id)
                    .requires_monomorphization(self.tcx)
                {
                    debug!("skipping function {} as it is generic", name);
                    self.record_region_gap(def_id, CoverageGapKind::SkippedRoot);
                    accounted_roots += 1;
                    continue;
                } else if self.tcx.is_const_fn(def_id) {
                    debug!("skipping function {} as it is a constant function", name);
                    self.record_region_gap(def_id, CoverageGapKind::SkippedRoot);
                    accounted_roots += 1;
                    continue;
                } else if utils::is_higher_order_function(def_id, self.tcx) {
                    debug!(
                        "skipping function {} as it is a higher order function",
                        name
                    );
                    self.record_region_gap(def_id, CoverageGapKind::SkippedRoot);
                    accounted_roots += 1;
                    continue;
                } else {
                    info!("analyzing function {}", name);
                }
            } else {
                info!("analyzing function {}", name);
            }
            self.call_graph.add_croot(def_id);
            self.analyze_body(def_id);
            accounted_roots += 1;
            if start_instant.elapsed().as_secs() >= self.options.max_analysis_time_for_crate {
                info!("exceeded total time allowed for crate analysis");
                for remaining_owner in &body_owners[body_index + 1..] {
                    self.record_region_gap(
                        remaining_owner.to_def_id(),
                        CoverageGapKind::CrateTimeout,
                    );
                }
                accounted_roots += body_owners.len() - body_index - 1;
                completed_enumeration = false;
                break;
            }
        }
        let envelope_sealed =
            envelope_is_sealed(completed_enumeration, accounted_roots, body_owners.len());
        self.emit_or_check_diagnostics(envelope_sealed)
    }

    fn record_region_gap(&mut self, def_id: DefId, kind: CoverageGapKind) {
        if !self.options.may_complete {
            return;
        }
        self.region_gaps.push(CoverageRecord::gap(
            self.tcx
                .sess
                .source_map()
                .span_to_diagnostic_string(self.tcx.def_span(def_id)),
            format!("{:?}", def_id),
            utils::summary_key_str(self.tcx, def_id).to_string(),
            kind,
        ));
    }

    /// Use compilation options to determine a list of functions to analyze.
    /// If this returns None, default logic is used by the caller.
    #[logfn(TRACE)]
    fn get_selected_function_list(&mut self) -> Option<Vec<String>> {
        if let Some(func_name) = &self.options.single_func {
            Some(vec![func_name.clone()])
        } else if self.options.test_only {
            // Extract test functions from the main test runner.
            if let Some((entry_def_id, _)) = self.tcx.entry_fn(()) {
                let fns = self.extract_test_fns(entry_def_id);
                if fns.is_empty() {
                    info!("Could not extract any tests from main entry point");
                } else {
                    info!("analyzing functions: {:?}", fns);
                }
                Some(fns)
            } else {
                warn!("Did not find main entry point to identify tests",);
                None
            }
        } else {
            None
        }
    }

    // Determine whether this function is included in the analysis.
    #[logfn(TRACE)]
    fn included_in(&self, list: &[String], name: &str, def_id: DefId) -> bool {
        let display_name = utils::def_id_display_name(self.tcx, def_id);
        // We check both for display name and summary key name.
        list.iter()
            .map(|e| e.as_str())
            .any(|e| e.eq(&display_name) || e.eq(name))
    }

    /// Run the abstract interpreter over the function body and produce a summary of its effects
    /// and collect any diagnostics into the buffer.
    #[logfn(TRACE)]
    fn analyze_body(&mut self, def_id: DefId) {
        let mut diagnostics: Vec<Diag<'compilation, ()>> = Vec::new();
        let mut coverage_records = Vec::new();
        let mut active_calls_map: HashMap<DefId, u64> = HashMap::new();
        let mut body_visitor = BodyVisitor::new(
            self,
            def_id,
            &mut diagnostics,
            &mut coverage_records,
            &mut active_calls_map,
            self.type_cache.clone(),
        );
        // Analysis local foreign contracts are not summarized and cached on demand, so we need to do it here.
        let summary = body_visitor.visit_body(&[]);
        let kind = self.tcx.def_kind(def_id);
        if matches!(kind, rustc_hir::def::DefKind::Static { .. })
            || utils::is_foreign_contract(self.tcx, def_id)
            || self.options.print_summaries
            || self.options.print_summaries_full
            || self.options.print_summaries_readable
        {
            self.summary_cache
                .set_summary_for(def_id, self.tcx, summary);
        }
        let old_diags = self.diagnostics_for.insert(def_id, diagnostics);
        checked_assume!(old_diags.is_none());
        let old_records = self.coverage_for.insert(def_id, coverage_records);
        checked_assume!(old_records.is_none());
    }

    /// Extract test functions from the promoted constants of a test runner main function.
    ///
    /// Currently, the #[test] attribute generates code like this:
    ///
    ///     extern crate test;
    ///
    ///     pub fn main() -> () {
    ///       test::test_main_static(&[&test, ...)...])
    ///     }
    ///
    ///     pub const test: test:TestDescAndFn = TestDecAndFn{
    ///       ...,
    ///       testfn: test::StaticTestFn(|| test::assert_test_result(test())
    ///     };
    ///
    ///     pub fn test() { <original user test code> }
    ///
    /// We can thus find the names (but not def def_id's) of the test functions in the main
    /// method (via const test in the example). However, the constant slice in main will be promoted
    /// into a constant initializer function in MIR, so we need to look there.
    /// We therefore iterate overall promoted functions belonging to the main function, looking for
    /// a statement like `_n = const <test name>` which loads the constant for a given test.
    ///
    /// This method is indeed not very stable and may break on changes to the compilation
    /// scheme or the test framework.
    fn extract_test_fns(&self, def_id: DefId) -> Vec<String> {
        let mut result = vec![];
        for body in self.tcx.promoted_mir(def_id).iter() {
            for b in body.basic_blocks.iter() {
                for s in &b.statements {
                    // The statement we are looking for has the form
                    // `Assign(_, Rvalue(Operand::Constant(ConstantKind::Unevaluated({def, ..}, _))))`
                    if let mir::StatementKind::Assign(box (
                        _,
                        mir::Rvalue::Use(mir::Operand::Constant(box ref con), _),
                    )) = s.kind
                    {
                        if let mir::Const::Unevaluated(c, _) = con.const_ {
                            result.push(utils::def_id_display_name(self.tcx, c.def));
                        }
                    }
                }
            }
        }
        result
    }

    /// Emit any diagnostics or, if testing, check that they are as expected.
    #[logfn_inputs(TRACE)]
    fn emit_or_check_diagnostics(&mut self, envelope_sealed: bool) -> bool {
        self.session.dcx().reset_err_count();
        let finding_count = self.diagnostics_for.values().flatten().count();
        let analyzed_bodies = self.coverage_for.len();
        if self.options.statistics {
            for (_, diags) in self.diagnostics_for.drain() {
                for db in diags.into_iter() {
                    db.cancel();
                }
            }
            print!("{}, analyzed, {}", self.file_name, finding_count);
        } else if self.test_run {
            let mut expected_errors = expected_errors::ExpectedErrors::new(self.file_name);
            let mut diags = vec![];
            for (_, dbs) in self.diagnostics_for.drain() {
                for db in dbs.into_iter() {
                    diags.push(db);
                }
            }
            let messages_ok = expected_errors.check_messages(&diags);
            for db in diags.into_iter() {
                db.cancel();
            }
            // Drain the coverage manifest before the (diverging) `fatal` below so a
            // failing expected-errors check cannot silently discard it.
            let coverage_is_clean =
                self.emit_coverage_manifest(analyzed_bodies, finding_count, envelope_sealed);
            if !messages_ok {
                self.session
                    .dcx()
                    .fatal(format!("test failed: {}", self.file_name));
            }
            return coverage_is_clean;
        } else {
            let mut diagnostics = vec![];
            for (_, dbs) in self.diagnostics_for.drain() {
                for db in dbs.into_iter() {
                    diagnostics.push(db);
                }
            }
            fn compare_diagnostics<'a>(x: &Diag<'a, ()>, y: &Diag<'a, ()>) -> Ordering {
                if x.span.primary_spans().lt(y.span.primary_spans()) {
                    Ordering::Less
                } else if x.span.primary_spans().gt(y.span.primary_spans()) {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
            diagnostics.sort_by(compare_diagnostics);
            for d in diagnostics.into_iter() {
                d.emit()
            }
        }
        self.emit_coverage_manifest(analyzed_bodies, finding_count, envelope_sealed)
    }

    fn emit_coverage_manifest(
        &mut self,
        analyzed_bodies: usize,
        finding_count: usize,
        envelope_sealed: bool,
    ) -> bool {
        if !self.options.may_complete {
            return true;
        }
        if self.options.statistics {
            println!();
        }
        let mut records = self
            .coverage_for
            .drain()
            .flat_map(|(_, records)| records)
            .chain(self.region_gaps.drain(..))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
        let outcome =
            CoverageOutcome::from_ledger(&records, analyzed_bodies, finding_count, envelope_sealed);
        let clean = outcome.is_clean();
        let gap_counts = count_gaps(&records);
        let crate_name = self.tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
        let manifest = CoverageManifest {
            crate_name: crate_name.as_str(),
            clean,
            outcome,
            envelope_sealed,
            analyzed_bodies,
            finding_count,
            gap_counts: &gap_counts,
            records: &records,
        };
        eprintln!(
            "MIRAI_COVERAGE_MANIFEST={}",
            serde_json::to_string(&manifest).expect("coverage manifest must serialize")
        );
        clean
    }

    pub fn print_summaries(&mut self) {
        if self.options.print_summaries_readable {
            let summaries = self.summary_cache.get_readable_summaries(self.tcx);
            print!("{}", summaries.to_json());
            return;
        }
        if self.options.print_summaries_full {
            let summaries = self.summary_cache.get_full_summaries(self.tcx);
            print!("{}", summaries.to_json());
            return;
        }
        if !self.options.print_summaries {
            return;
        }
        let calls_for_def_ids = self.call_graph.get_calls_for_def_ids();
        let summaries_for_llm = self
            .summary_cache
            .get_summaries_for_llm(self.tcx, calls_for_def_ids);
        print!("{}", summaries_for_llm.to_json());
    }
}
