// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::env::current_dir;
use std::fmt::{Debug, Formatter, Result};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;

use itertools::Itertools;
use log::trace;
use log_derive::{logfn, logfn_inputs};
use serde::{Deserialize, Serialize};
use sled::{Config, Db};

use mirai_annotations::*;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::{Ty, TyCtxt};
use rustc_span::Span;

use crate::abstract_value::AbstractValueTrait;
use crate::abstract_value::{self, AbstractValue};
use crate::constant_domain::FunctionReference;
use crate::environment::Environment;
use crate::expression::Expression;
use crate::path::{Path, PathEnum, PathRefinement, PathRoot, PathSelector};
use crate::utils;

/// A summary is a declarative abstract specification of what a function does.
/// This is calculated once per function and is used by callers of the function.
/// Callers will specialize this summary by replacing embedded parameter values with the corresponding
/// argument values and then simplifying the resulting values under the current path condition.
///
/// Summaries are stored in a persistent, per project database. When a crate is recompiled,
/// all summaries arising from the crate are recomputed, but the database is only updated when
/// a function summary changes. When this happens, all callers of the function need to be reanalyzed
/// using the new summary, which could result in their summaries being updated, and so on.
/// In the case of recursive loops, a function summary may need to be recomputed and widened
/// until a fixed point is reached. Since crate dependencies are acyclic, fixed point computation
/// can be limited to the functions of one crate and the dependency graph need not be stored
/// in the database.
///
/// There are three ways summaries are constructed:
/// 1) By analyzing the body of the actual function.
/// 2) By analyzing the body of a contract function that contains only enough code to generate
///    an accurate summary. This is the preferred way to deal with abstract and foreign functions
///    where the actual body is not known until runtime.
/// 3) By constructing a dummy summary using only the type signature of the function.
///    In such cases there are no preconditions, no post conditions, the result value is fully
///    abstract as is the unwind condition and the values assigned to any mutable parameter.
///    This makes the summary a conservative over approximation of the actual behavior. It is not
///    sound, however, if there are side effects on static state since it is neither practical nor
///    desirable to havoc all static variables every time such a function is called. Consequently
///    sound analysis is only possible if one can assume that all such functions have been provided
///    with explicit contract functions.
#[derive(Serialize, Deserialize, Clone, Debug, Default, Eq, PartialEq)]
pub struct Summary {
    /// If true this summary was computed. If false, it is a default summary.
    /// Used to distinguish a computed empty summary from a default summary.
    /// In the latter case, the summary should be computed from MIR, if available.
    /// If no MIR is available, the summary is left empty but marked as is_computed so that
    /// there are no repeated attempts at recomputing the summary.
    pub is_computed: bool,

    /// If true, the summary is incomplete, which means that the result and side effects could be
    /// over specific because widening did not happen and also that some side effects may be missing.
    /// The summary may also fail to mention necessary preconditions or useful post conditions.
    /// This happens if the computation of this summary failed for some reason, for example
    /// no MIR body or a time-out.
    /// A function that makes use of an incomplete summary cannot be fully analyzed and thus becomes
    /// incomplete in turn.
    pub is_incomplete: bool,

    // Conditions that should hold prior to the call.
    // Callers should substitute parameter values with argument values and simplify the results
    // under the current path condition. Any values that do not simplify to true will require the
    // caller to either generate an error message or to add a precondition to its own summary that
    // will be sufficient to ensure that all the preconditions in this summary are met.
    // The string value bundled with the condition is the message that details what would go
    // wrong at runtime if the precondition is not satisfied by the caller.
    pub preconditions: Vec<Precondition>,

    /// Pairs of pointer-like paths whose dereference targets are the same allocation.
    #[serde(default)]
    pub assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,

    /// Alias relationships that hold when the associated condition is true.
    #[serde(default)]
    pub guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,

    // Modifications the function makes to mutable state external to the function.
    // Every path will be rooted in a static or in a mutable parameter.
    // No two paths in this collection will lead to the same place in memory.
    // Callers should substitute parameter values with argument values and simplify the results
    // under the current path condition. They should then update their current state to reflect the
    // side effects of the call.
    pub side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,

    // A condition that should hold after a call that completes normally.
    // Callers should substitute parameter values with argument values and simplify the results
    // under the current path condition.
    // The resulting value should be conjoined to the current path condition.
    pub post_condition: Option<Rc<AbstractValue>>,

    /// The type table index for the Rust type of the actual return value.
    /// Used to make type tracking more precise when the body returns a value of concrete type
    /// but the return type specification is abstract.
    #[serde(skip)]
    pub return_type_index: usize,

    /// Calls to function-typed parameters, together with their boundary-visible state.
    #[serde(default)]
    pub callback_invocations: Vec<CallbackInvocation>,

    /// Model-field values established on normal return after callbacks. These are kept separate
    /// from ordinary side effects so they can survive incomplete callback replay without exposing
    /// arbitrary partial state.
    #[serde(default)]
    pub incomplete_model_state: Vec<(Rc<Path>, Rc<AbstractValue>)>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CallbackInvocation {
    /// The function-typed parameter invoked by the summarized function.
    pub callee: Rc<Path>,
    /// Callback arguments expressed in terms of the summarized function's parameters. An
    /// unavailable argument is represented by BOTTOM and marks the invocation incomplete.
    pub arguments: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    /// False if any callback argument could not be expressed in the summary's path namespace.
    #[serde(default = "complete_callback_arguments")]
    pub arguments_complete: bool,
    /// Boundary-refinable field values visible at the callback invocation point. Projections of an
    /// otherwise unavailable callback argument use a synthetic argument root.
    pub pre_state: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    /// Boundary-visible alias relationships established before the callback invocation.
    #[serde(default)]
    pub pre_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    /// Boundary-visible conditional alias relationships established before the callback invocation.
    #[serde(default)]
    pub pre_guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    /// The path condition under which the callback is invoked.
    pub guard: Rc<AbstractValue>,
    /// The resolved callback specialization, when one was available while summarizing the call.
    pub specialized_callee: Option<Rc<FunctionReference>>,
    /// Transitive function constants used to specialize the callback summary.
    pub function_constants: Vec<Rc<FunctionReference>>,
    /// True when the invoked closure is local to the summarized function and is anchored to one
    /// of its captured parameters for serialization.
    #[serde(default)]
    pub is_local: bool,
    /// Stable identity of the callback chain that produced this invocation.
    pub carrier_id: Option<u64>,
    /// Analysis-local path rekeys used to express state established after the callback in the
    /// summary namespace. The resulting state is serialized on the enclosing Summary instead.
    #[serde(skip)]
    pub state_rekeys: Vec<(Rc<Path>, Rc<AbstractValue>)>,
}

fn complete_callback_arguments() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct PreIncompleteModelStateSummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<Precondition>,
    assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    post_condition: Option<Rc<AbstractValue>>,
    callback_invocations: Vec<CallbackInvocation>,
}

impl From<PreIncompleteModelStateSummary> for Summary {
    fn from(summary: PreIncompleteModelStateSummary) -> Self {
        Summary {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary.preconditions,
            assumed_aliases: summary.assumed_aliases,
            guarded_aliases: summary.guarded_aliases,
            side_effects: summary.side_effects,
            post_condition: summary.post_condition,
            return_type_index: 0,
            callback_invocations: summary.callback_invocations,
            incomplete_model_state: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PreLineageCallbackInvocation {
    callee: Rc<Path>,
    arguments: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    arguments_complete: bool,
    pre_state: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    pre_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    pre_guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    guard: Rc<AbstractValue>,
    specialized_callee: Option<Rc<FunctionReference>>,
    function_constants: Vec<Rc<FunctionReference>>,
    is_local: bool,
}

#[derive(Serialize, Deserialize)]
struct PreLineageSummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<Precondition>,
    assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    post_condition: Option<Rc<AbstractValue>>,
    callback_invocations: Vec<PreLineageCallbackInvocation>,
}

impl From<PreLineageSummary> for Summary {
    fn from(summary: PreLineageSummary) -> Self {
        Summary {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary.preconditions,
            assumed_aliases: summary.assumed_aliases,
            guarded_aliases: summary.guarded_aliases,
            side_effects: summary.side_effects,
            post_condition: summary.post_condition,
            return_type_index: 0,
            callback_invocations: summary
                .callback_invocations
                .into_iter()
                .map(|invocation| CallbackInvocation {
                    callee: invocation.callee,
                    arguments: invocation.arguments,
                    arguments_complete: invocation.arguments_complete,
                    pre_state: invocation.pre_state,
                    pre_aliases: invocation.pre_aliases,
                    pre_guarded_aliases: invocation.pre_guarded_aliases,
                    guard: invocation.guard,
                    specialized_callee: invocation.specialized_callee,
                    function_constants: invocation.function_constants,
                    is_local: invocation.is_local,
                    carrier_id: None,
                    state_rekeys: Vec::new(),
                })
                .collect(),
            incomplete_model_state: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PreAliasCallbackInvocation {
    callee: Rc<Path>,
    arguments: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    arguments_complete: bool,
    pre_state: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    guard: Rc<AbstractValue>,
    specialized_callee: Option<Rc<FunctionReference>>,
    function_constants: Vec<Rc<FunctionReference>>,
    is_local: bool,
}

#[derive(Serialize, Deserialize)]
struct PreAliasSummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<Precondition>,
    assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    post_condition: Option<Rc<AbstractValue>>,
    callback_invocations: Vec<PreAliasCallbackInvocation>,
}

impl From<PreAliasSummary> for Summary {
    fn from(summary: PreAliasSummary) -> Self {
        Summary {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary.preconditions,
            assumed_aliases: summary.assumed_aliases,
            guarded_aliases: summary.guarded_aliases,
            side_effects: summary.side_effects,
            post_condition: summary.post_condition,
            return_type_index: 0,
            callback_invocations: summary
                .callback_invocations
                .into_iter()
                .map(|invocation| CallbackInvocation {
                    callee: invocation.callee,
                    specialized_callee: invocation.specialized_callee,
                    function_constants: invocation.function_constants,
                    arguments: invocation.arguments,
                    arguments_complete: invocation.arguments_complete,
                    pre_state: invocation.pre_state,
                    pre_aliases: Vec::new(),
                    pre_guarded_aliases: Vec::new(),
                    guard: invocation.guard,
                    is_local: invocation.is_local,
                    carrier_id: None,
                    state_rekeys: Vec::new(),
                })
                .collect(),
            incomplete_model_state: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct PreviousCallbackInvocation {
    callee: Rc<Path>,
    arguments: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    pre_state: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    guard: Rc<AbstractValue>,
}

#[derive(Deserialize)]
struct OlderCallbackInvocation {
    callee: Rc<Path>,
    arguments: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    pre_state: Vec<(Rc<Path>, Rc<AbstractValue>)>,
}

#[derive(Deserialize)]
struct PreviousSummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<Precondition>,
    assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    post_condition: Option<Rc<AbstractValue>>,
    callback_invocations: Vec<PreviousCallbackInvocation>,
}

impl From<PreviousSummary> for Summary {
    fn from(summary: PreviousSummary) -> Self {
        Summary {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary.preconditions,
            assumed_aliases: summary.assumed_aliases,
            guarded_aliases: summary.guarded_aliases,
            side_effects: summary.side_effects,
            post_condition: summary.post_condition,
            return_type_index: 0,
            callback_invocations: summary
                .callback_invocations
                .into_iter()
                .map(|invocation| CallbackInvocation {
                    callee: invocation.callee,
                    specialized_callee: None,
                    function_constants: Vec::new(),
                    arguments: invocation.arguments,
                    arguments_complete: true,
                    pre_state: invocation.pre_state,
                    pre_aliases: Vec::new(),
                    pre_guarded_aliases: Vec::new(),
                    guard: invocation.guard,
                    is_local: false,
                    carrier_id: None,
                    state_rekeys: Vec::new(),
                })
                .collect(),
            incomplete_model_state: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct OlderSummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<Precondition>,
    assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    post_condition: Option<Rc<AbstractValue>>,
    callback_invocations: Vec<OlderCallbackInvocation>,
}

impl From<OlderSummary> for Summary {
    fn from(summary: OlderSummary) -> Self {
        Summary {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary.preconditions,
            assumed_aliases: summary.assumed_aliases,
            guarded_aliases: summary.guarded_aliases,
            side_effects: summary.side_effects,
            post_condition: summary.post_condition,
            return_type_index: 0,
            callback_invocations: summary
                .callback_invocations
                .into_iter()
                .map(|invocation| CallbackInvocation {
                    callee: invocation.callee,
                    specialized_callee: None,
                    function_constants: Vec::new(),
                    arguments: invocation.arguments,
                    arguments_complete: true,
                    pre_state: invocation.pre_state,
                    pre_aliases: Vec::new(),
                    pre_guarded_aliases: Vec::new(),
                    guard: Rc::new(abstract_value::TRUE),
                    is_local: false,
                    carrier_id: None,
                    state_rekeys: Vec::new(),
                })
                .collect(),
            incomplete_model_state: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
struct LegacySummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<Precondition>,
    assumed_aliases: Vec<(Rc<Path>, Rc<Path>)>,
    guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)>,
    side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,
    post_condition: Option<Rc<AbstractValue>>,
}

impl From<LegacySummary> for Summary {
    fn from(summary: LegacySummary) -> Self {
        Summary {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary.preconditions,
            assumed_aliases: summary.assumed_aliases,
            guarded_aliases: summary.guarded_aliases,
            side_effects: summary.side_effects,
            post_condition: summary.post_condition,
            return_type_index: 0,
            callback_invocations: Vec::new(),
            incomplete_model_state: Vec::new(),
        }
    }
}

fn deserialize_summary(bytes: &[u8]) -> bincode::Result<Summary> {
    bincode::deserialize(bytes)
        .or_else(|_| bincode::deserialize::<PreIncompleteModelStateSummary>(bytes).map(Into::into))
        .or_else(|_| bincode::deserialize::<PreLineageSummary>(bytes).map(Into::into))
        .or_else(|_| bincode::deserialize::<PreAliasSummary>(bytes).map(Into::into))
        .or_else(|_| bincode::deserialize::<PreviousSummary>(bytes).map(Into::into))
        .or_else(|_| bincode::deserialize::<OlderSummary>(bytes).map(Into::into))
        .or_else(|_| bincode::deserialize::<LegacySummary>(bytes).map(Into::into))
}

/// Bundles together the condition of a precondition with the provenance (place where defined) of
/// the condition, along with a diagnostic message to use when the precondition is not (might not be)
/// satisfied.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Precondition {
    /// The condition that must be satisfied when calling a function that has this precondition.
    pub condition: Rc<AbstractValue>,
    /// A diagnostic message to issue if the precondition is not met.
    pub message: Rc<str>,
    /// The source location of the precondition definition (or the source expression/statement that
    /// would panic if the precondition is not met). This is in textual form because it needs to be
    /// persistable and crate independent.
    pub provenance: Option<Rc<str>>,
    /// A stack of source locations that lead to the definition of the precondition (or the source
    /// expression/statement that would panic if the precondition is not met). It is a stack
    /// because the precondition might have been promoted (when a non-public function does not meet
    /// a precondition of a function it calls, MIRAI infers a precondition that will allow it to
    /// meet the precondition of the call, so things stack up).
    /// Because this situation arises for non-public functions, it is possible to use source spans
    /// rather than strings to track the locations where the promotions happen.
    #[serde(skip)]
    pub spans: Vec<rustc_span::Span>,
}

impl Summary {
    #[logfn_inputs(TRACE)]
    pub fn is_subset_of(&self, other: &Summary) -> bool {
        if !Self::is_subset_of_preconditions(&self.preconditions[0..], &other.preconditions[0..]) {
            return false;
        }
        if !Self::is_subset_of_side_effects(&self.side_effects[0..], &other.side_effects[0..]) {
            return false;
        }
        if !self
            .assumed_aliases
            .iter()
            .all(|alias| other.assumed_aliases.contains(alias))
        {
            return false;
        }
        if !self
            .guarded_aliases
            .iter()
            .all(|alias| other.guarded_aliases.contains(alias))
        {
            return false;
        }
        if !self
            .callback_invocations
            .iter()
            .all(|invocation| other.callback_invocations.contains(invocation))
        {
            return false;
        }
        true
    }

    #[logfn_inputs(TRACE)]
    fn is_subset_of_preconditions(p1: &[Precondition], p2: &[Precondition]) -> bool {
        if p1.is_empty() {
            return true;
        }
        if p2.is_empty() {
            return false;
        }
        if p1[0].spans < p2[0].spans {
            return false;
        }
        if p1[0].spans > p2[0].spans {
            return Self::is_subset_of_preconditions(p1, &p2[1..]);
        }
        if !p1[0].condition.subset(&p2[0].condition) {
            return false;
        }
        Self::is_subset_of_preconditions(&p1[1..], &p2[1..])
    }

    #[logfn_inputs(TRACE)]
    fn is_subset_of_side_effects(
        e1: &[(Rc<Path>, Rc<AbstractValue>)],
        e2: &[(Rc<Path>, Rc<AbstractValue>)],
    ) -> bool {
        if e1.is_empty() {
            return true;
        }
        if e2.is_empty() {
            return false;
        }
        let (p1, v1) = &e1[0];
        let (p2, v2) = &e2[0];
        if p1 < p2 {
            return false;
        }
        if p1 > p2 {
            return Self::is_subset_of_side_effects(e1, &e2[1..]);
        }
        if !v1.subset(v2) {
            return false;
        }
        Self::is_subset_of_side_effects(&e1[1..], &e2[1..])
    }

    pub fn join_side_effects(&mut self, other: &Summary) {
        // Aliasing is a must-property, so retain only relationships present on both iterations.
        self.assumed_aliases
            .retain(|alias| other.assumed_aliases.contains(alias));
        self.guarded_aliases
            .retain(|alias| other.guarded_aliases.contains(alias));
        for invocation in &other.callback_invocations {
            if !self.callback_invocations.contains(invocation) {
                self.callback_invocations.push(invocation.clone());
            }
        }
        let other_map: HashMap<Rc<Path>, Rc<AbstractValue>> =
            other.side_effects.clone().into_iter().collect();
        for (path, val1) in self.side_effects.iter_mut() {
            match other_map.get(path) {
                Some(val2) => {
                    *val1 = val1.join((*val2).clone());
                }
                None => {
                    if path.is_rooted_by_parameter() {
                        let val2 = AbstractValue::make_initial_parameter_value(
                            val1.expression.infer_type(),
                            path.clone(),
                        );
                        *val1 = val1.join(val2);
                    };
                }
            }
        }
    }

    pub fn widen_side_effects(&mut self) {
        for (path, value) in self.side_effects.iter_mut() {
            *value = value.widen(path);
        }
    }
}

/// Constructs a summary of a function body by processing state information gathered during
/// abstract interpretation of the body.
#[allow(clippy::too_many_arguments)]
#[logfn(TRACE)]
pub fn summarize(
    argument_count: usize,
    exit_environment: Option<&Environment>,
    preconditions: &[Precondition],
    callback_invocations: &[CallbackInvocation],
    post_condition: &Option<Rc<AbstractValue>>,
    return_type_index: usize,
    tcx: TyCtxt<'_>,
) -> Summary {
    trace!(
        "summarize env {:?} pre {:?} post {:?}",
        exit_environment,
        preconditions,
        post_condition,
    );
    let mut preconditions: Vec<Precondition> = add_provenance(preconditions, tcx);
    let mut assumed_aliases: Vec<(Rc<Path>, Rc<Path>)> = exit_environment
        .map(|environment| environment.assumed_aliases.iter())
        .into_iter()
        .flatten()
        .filter(|(alias, source)| {
            let is_boundary_path = |path: &Rc<Path>| {
                matches!(
                    path.get_path_root().value,
                    PathEnum::Parameter { .. } | PathEnum::Result | PathEnum::StaticVariable { .. }
                )
            };
            is_boundary_path(alias) && is_boundary_path(source)
        })
        .cloned()
        .collect();
    let mut guarded_aliases: Vec<(Rc<Path>, Rc<Path>, Rc<AbstractValue>)> = exit_environment
        .map(|environment| environment.guarded_aliases.iter())
        .into_iter()
        .flatten()
        .filter(|((alias, source), _)| {
            let is_boundary_path = |path: &Rc<Path>| {
                matches!(
                    path.get_path_root().value,
                    PathEnum::Parameter { .. } | PathEnum::Result | PathEnum::StaticVariable { .. }
                )
            };
            is_boundary_path(alias) && is_boundary_path(source)
        })
        .filter_map(|((alias, source), condition)| {
            condition
                .extract_promotable_disjuncts(false)
                .map(|condition| (alias.clone(), source.clone(), condition))
        })
        .collect();
    let mut side_effects = if let Some(exit_environment) = exit_environment {
        extract_side_effects(exit_environment, argument_count)
    } else {
        vec![]
    };
    let incomplete_model_state = exit_environment
        .map(|environment| extract_incomplete_model_state(&[environment], callback_invocations))
        .unwrap_or_default();

    preconditions.sort();
    assumed_aliases.sort();
    guarded_aliases.sort();
    side_effects.sort();

    Summary {
        is_computed: true,
        is_incomplete: false,
        preconditions,
        assumed_aliases,
        guarded_aliases,
        side_effects,
        post_condition: post_condition.clone(),
        return_type_index,
        callback_invocations: callback_invocations.to_vec(),
        incomplete_model_state,
    }
}

fn extract_incomplete_model_state(
    environments: &[&Environment],
    callback_invocations: &[CallbackInvocation],
) -> Vec<(Rc<Path>, Rc<AbstractValue>)> {
    let mut incomplete_model_state = environments
        .iter()
        .flat_map(|environment| {
            environment
                .value_map
                .iter()
                .filter(|(path, _)| {
                    matches!(
                        path.value,
                        PathEnum::QualifiedPath { ref selector, .. }
                            if matches!(**selector, PathSelector::ModelField(_))
                    )
                })
                .map(move |(path, value)| (*environment, path.clone(), value.clone()))
        })
        .filter_map(|(environment, path, value)| {
            let state_rekeys = callback_invocations
                .iter()
                .flat_map(|invocation| &invocation.state_rekeys)
                .collect::<Vec<_>>();
            let path = if let Some((_, replacement)) = state_rekeys
                .iter()
                .find(|(local_path, _)| **local_path == *path)
            {
                Path::get_as_path((*replacement).clone())
            } else {
                state_rekeys
                    .into_iter()
                    .filter(|(local_path, _)| {
                        !matches!(
                            local_path.value,
                            PathEnum::QualifiedPath { ref selector, .. }
                                if matches!(**selector, PathSelector::ModelField(_))
                        )
                    })
                    .fold(path, |path, (local_path, replacement)| {
                        let replacement =
                            Path::get_as_path(replacement.clone()).canonicalize(environment);
                        path.replace_root(local_path, replacement)
                    })
            };
            let path = environment.canonicalize_model_field_path(path);
            let is_callback_state_path = callback_invocations.iter().any(|invocation| {
                invocation
                    .pre_state
                    .iter()
                    .any(|(pre_state_path, _)| *pre_state_path == path)
            });
            ((path.is_rooted_by_parameter() || is_callback_state_path)
                && !path.contains_local_variable(false)
                && !value.expression.contains_local_variable(false)
                && matches!(
                    path.value,
                    PathEnum::QualifiedPath { ref selector, .. }
                        if matches!(**selector, PathSelector::ModelField(_))
                )
                && matches!(
                    &value.expression,
                    Expression::CompileTimeConstant(constant) if constant.is_zero()
                ))
            .then_some((path, value))
        })
        .collect::<Vec<_>>();
    incomplete_model_state.sort();
    incomplete_model_state.dedup();
    incomplete_model_state
}

/// Constructs the sound subset of a summary after analysis stopped at an unresolved operation.
/// Preconditions and callback invocations recorded before the failure remain valid requirements.
/// Partial side effects, model state, and postconditions are not safe to expose to callers.
pub(crate) fn retain_in_incomplete_summary(precondition: &Precondition) -> bool {
    !precondition
        .message
        .starts_with("incomplete analysis of call")
}

#[logfn(TRACE)]
pub fn summarize_incomplete(
    _argument_count: usize,
    _current_environment: &Environment,
    preconditions: &[Precondition],
    callback_invocations: &[CallbackInvocation],
    _exit_environment: Option<&Environment>,
    tcx: TyCtxt<'_>,
) -> Summary {
    trace!(
        "summarize_incomplete input preconditions {:?} callback_invocations {:?}",
        preconditions,
        callback_invocations
    );
    let mut preconditions = add_provenance(
        &preconditions
            .iter()
            .filter(|precondition| retain_in_incomplete_summary(precondition))
            .cloned()
            .collect::<Vec<_>>(),
        tcx,
    );
    preconditions.sort();
    Summary {
        is_computed: true,
        is_incomplete: true,
        preconditions,
        callback_invocations: callback_invocations.to_vec(),
        incomplete_model_state: Vec::new(),
        ..Summary::default()
    }
}

/// When a precondition is being serialized into a summary, it needs a provenance that is not
/// specific to the current (crate) compilation, since the summary may be used to compile a different
/// crate, or a different version of the current crate.
#[logfn(TRACE)]
fn add_provenance(preconditions: &[Precondition], tcx: TyCtxt<'_>) -> Vec<Precondition> {
    preconditions
        .iter()
        .map(|precondition| {
            let mut precond = precondition.clone();
            if !precondition.spans.is_empty() {
                let last_span = precondition.spans.last();
                let span = last_span.unwrap().source_callsite();
                precond.provenance = Some(Rc::from(
                    tcx.sess
                        .source_map()
                        .span_to_diagnostic_string(span)
                        .as_str(),
                ));
            }
            precond
        })
        .collect()
}

/// Returns a list of (path, value) pairs where each path is rooted by an argument(or the result)
/// or where the path root is a heap block reachable from an argument (or the result).
/// Since paths are created by writes, these are side effects.
/// Since these values are reachable from arguments or the result, they are visible to the caller
/// and must be included in the summary.
#[logfn_inputs(TRACE)]
fn extract_side_effects(
    env: &Environment,
    argument_count: usize,
) -> Vec<(Rc<Path>, Rc<AbstractValue>)> {
    let mut heap_roots: HashSet<Rc<AbstractValue>> = HashSet::new();
    let mut result = Vec::new();
    for ordinal in 0..=argument_count {
        let root = if ordinal == 0 {
            Path::new_result()
        } else {
            Path::new_parameter(ordinal)
        };
        for (path, value) in env
            .value_map
            .iter()
            .filter(|(p, _)| (ordinal == 0 && (**p) == root) || p.is_rooted_by(&root))
            .sorted_by(|(p1, _), (p2, _)| {
                let len1 = p1.path_length();
                let len2 = p2.path_length();
                if len1 == len2 {
                    if matches!(&p1.value, PathEnum::QualifiedPath { selector, .. } if **selector == PathSelector::Deref) {
                        Ordering::Less
                    } else {
                        Ordering::Equal
                    }
                } else {
                    len1.cmp(&len2)
                }
            })
        {
            path.record_heap_blocks_and_strings(&mut heap_roots);
            value.record_heap_blocks_and_strings(&mut heap_roots);
            if let Expression::Variable { path: vpath, .. } | Expression::InitialParameterValue { path: vpath, .. } = &value.expression {
                if ordinal > 0 && vpath.eq(path) {
                    // The value is not an update, but just what was there at function entry.
                    continue;
                }
            }
            result.push((path.clone(), value.clone()));
        }
    }
    extract_reachable_heap_allocations(env, &mut heap_roots, &mut result);
    result
}

/// Adds roots for all new heap allocated objects that are reachable by the caller.
#[logfn_inputs(TRACE)]
fn extract_reachable_heap_allocations(
    env: &Environment,
    heap_roots: &mut HashSet<Rc<AbstractValue>>,
    result: &mut Vec<(Rc<Path>, Rc<AbstractValue>)>,
) {
    let mut visited_heap_roots: HashSet<Rc<AbstractValue>> = HashSet::new();
    while heap_roots.len() > visited_heap_roots.len() {
        let mut new_roots: HashSet<Rc<AbstractValue>> = HashSet::new();
        for heap_root in heap_roots.iter() {
            if visited_heap_roots.insert(heap_root.clone()) {
                let root = Path::get_as_path(heap_root.clone());
                for (path, value) in env
                    .value_map
                    .iter()
                    .filter(|(p, _)| (**p) == root || p.is_rooted_by(&root))
                {
                    path.record_heap_blocks_and_strings(&mut new_roots);
                    value.record_heap_blocks_and_strings(&mut new_roots);
                    result.push((path.clone(), value.clone()));
                }
            }
        }
        heap_roots.extend(new_roots);
    }
}

/// If a call site provides type arguments to a generic function, or if some of the arguments
/// are constant functions, the function summary used at the call site needs to be specialized
/// with respect to these arguments and when we store summaries in a cache we need the cache
/// key to be based on these arguments.
#[derive(PartialEq, Eq)]
pub struct CallSiteKey<'tcx> {
    /// If this is None, type_args must not be None.
    func_args: Option<Rc<Vec<Rc<FunctionReference>>>>,
    /// If this is None, func_args must not be None.
    type_args: Option<Rc<HashMap<Rc<Path>, Ty<'tcx>>>>,
    /// Uniquely identifies the function reference used at the call site.
    function_id: usize,
}

impl<'tcx> CallSiteKey<'tcx> {
    pub fn new(
        func_args: Option<Rc<Vec<Rc<FunctionReference>>>>,
        type_args: Option<Rc<HashMap<Rc<Path>, Ty<'tcx>>>>,
        function_id: usize,
    ) -> CallSiteKey<'tcx> {
        CallSiteKey {
            func_args,
            type_args,
            function_id,
        }
    }
}

impl Hash for CallSiteKey<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(func_args) = &self.func_args {
            func_args.hash(state);
        }
        if let Some(cache) = &self.type_args {
            for (path, ty) in cache.iter() {
                path.hash(state);
                ty.kind().hash(state);
            }
        }
        self.function_id.hash(state);
    }
}

/// A database and collection of in-memory caches for function summaries.
pub struct SummaryCache<'tcx> {
    /// The sled database that stores the summaries when persisted between runs.
    /// Chiefly used to store summaries for rust standard library functions that have no MIR.
    db: Db,
    /// Functions that are entry points have def_ids but no function_id, because they are not
    /// derived from function references, have their summaries cached here.
    def_id_cache: HashMap<DefId, Summary>,
    /// Functions that are summarized because they are called via function references, have their
    /// summaries cached here. These summaries will be specialized using the generic arguments (if any)
    /// supplied by the function reference.
    function_id_cache: HashMap<usize, Summary>,
    /// Maps call sites to specialized summaries of the referenced functions.
    /// Call site specialization involves using the actual generic type arguments supplied by the call
    /// site, along with the values of any constant functions that are supplied as actual arguments.
    /// This cache is only used if the call site supplies generic type arguments or constant functions.
    call_site_cache: HashMap<CallSiteKey<'tcx>, Summary>,
    /// Functions that have no def_id (and hence no function_id) and no type signature are
    /// cached here. Such functions are either entry points or dummy functions that provide
    /// summaries for functions that have no MIR and are shadowed by definitions in a contracts crate.
    reference_cache: HashMap<Rc<FunctionReference>, Summary>,
    /// A cache of summary keys for each def_id. This is used to avoid recomputing the summary key,
    /// which is expensive to do and can be done more than once per def_id if there are more than
    /// one call site that references the def_id.
    key_cache: HashMap<DefId, Rc<str>>,
}

impl Debug for SummaryCache<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        "SummaryCache".fmt(f)
    }
}

impl<'tcx> SummaryCache<'tcx> {
    /// Creates a new summary cache, using (or creating) a Sled database at the given directory path.
    #[logfn(TRACE)]
    pub fn new(summary_store_directory_str: String) -> SummaryCache<'tcx> {
        use rand::{rng, Rng};
        use std::thread;
        use std::time::Duration;

        let mut rng = rng();
        let summary_store_path = Self::create_summary_store_if_needed(&summary_store_directory_str);
        let config = Config::default().path(summary_store_path);
        let mut result;
        loop {
            result = config.open();
            if result.is_ok() {
                break;
            }
            debug!("opening db failed {:?}", result);
            let num_millis = rng.random_range(100..200);
            thread::sleep(Duration::from_millis(num_millis));
        }
        let db = result.unwrap_or_else(|err| {
            debug!("{} ", err);
            assume_unreachable!();
        });
        SummaryCache {
            db,
            def_id_cache: HashMap::new(),
            function_id_cache: HashMap::new(),
            call_site_cache: HashMap::new(),
            reference_cache: HashMap::new(),
            key_cache: HashMap::new(),
        }
    }

    /// Creates a Sled database at the given directory path, if it does not already exist.
    /// The initial value of the database contains summaries of standard library functions.
    /// The code used to create these summaries are mirai/standard_contracts.
    #[logfn_inputs(TRACE)]
    fn create_summary_store_if_needed(summary_store_directory_str: &str) -> std::path::PathBuf {
        use std::env;
        use std::fs::OpenOptions;
        use std::io::Cursor;
        use std::path::Path;

        use fs2::FileExt;
        use tar::Archive;

        let directory_path = Path::new(summary_store_directory_str);
        let store_path = directory_path.join(".summary_store.sled");
        if env::var("MIRAI_START_FRESH").is_ok() {
            std::fs::remove_dir_all(directory_path).unwrap();
            std::fs::create_dir_all(directory_path).unwrap();
        } else if !store_path.exists() {
            std::fs::create_dir_all(directory_path).unwrap();
            let initialization_lock = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(directory_path.join(".summary_store.init.lock"))
                .unwrap();
            initialization_lock.lock_exclusive().unwrap();
            if !store_path.exists() {
                info!("creating a new summary store from the embedded tar file");
                let bytes = include_bytes!("../../binaries/summary_store.tar");
                let mut ar = Archive::new(Cursor::new(bytes));
                ar.unpack(directory_path).unwrap();
            }
        }
        store_path
    }

    pub fn get_summaries_for_llm(
        &self,
        tcx: TyCtxt,
        call_site_per_def_id: HashMap<DefId, Vec<(Span, DefId)>>,
    ) -> SummariesForLLM {
        let source_map = tcx.sess.source_map();
        let mut entries = Vec::new();
        for (key, value) in self.def_id_cache.iter() {
            let fully_qualified_name = self.key_cache.get(key).unwrap().to_string();
            let (path, source) = Self::get_source(tcx, *key);
            let mut calls = vec![];
            if let Some(call_vec) = call_site_per_def_id.get(key) {
                for (span, def_id) in call_vec.iter() {
                    let call_snippet = source_map.span_to_snippet(*span).ok().unwrap_or_default();
                    let callee_name = self.key_cache.get(def_id).unwrap().to_string();
                    calls.push((call_snippet, callee_name));
                }
            };
            entries.push((
                path,
                fully_qualified_name,
                source,
                LLMSummary::from_summary(value, calls),
            ));
        }
        SummariesForLLM { entries }
    }

    pub fn get_full_summaries(&self, tcx: TyCtxt) -> FullSummaries {
        let entries = self
            .def_id_cache
            .iter()
            .map(|(def_id, summary)| {
                let (path, source) = Self::get_source(tcx, *def_id);
                let fully_qualified_name = self.key_cache.get(def_id).unwrap().to_string();
                (path, fully_qualified_name, source, summary.clone())
            })
            .collect();
        FullSummaries { entries }
    }

    pub fn get_readable_summaries(&self, tcx: TyCtxt) -> ReadableSummaries {
        let entries = self
            .def_id_cache
            .iter()
            .map(|(def_id, summary)| {
                let (path, source) = Self::get_source(tcx, *def_id);
                let fully_qualified_name = self.key_cache.get(def_id).unwrap().to_string();
                (
                    path,
                    fully_qualified_name,
                    source,
                    ReadableSummary::from(summary),
                )
            })
            .collect();
        ReadableSummaries { entries }
    }

    fn get_source(tcx: TyCtxt, def_id: DefId) -> (String, String) {
        let source_map = tcx.sess.source_map();
        let (file_name, source) = if tcx.is_mir_available(def_id) {
            let mir = if tcx.is_const_fn(def_id) {
                tcx.mir_for_ctfe(def_id)
            } else {
                let instance = rustc_middle::ty::InstanceKind::Item(def_id);
                tcx.instance_mir(instance)
            };
            (
                source_map.span_to_filename(mir.span).into_local_path(),
                source_map
                    .span_to_snippet(mir.span)
                    .ok()
                    .unwrap_or_default(),
            )
        } else {
            let span = tcx.def_span(def_id);
            (
                source_map.span_to_filename(span).into_local_path(),
                source_map.span_to_snippet(span).ok().unwrap_or_default(),
            )
        };
        let Some(mut path) = file_name else {
            return (String::new(), source);
        };
        if path.is_absolute() {
            path = path
                .strip_prefix(current_dir().unwrap_or_default())
                .unwrap_or(&path)
                .to_path_buf();
            if path.is_absolute() {
                let sysroot = utils::find_sysroot();
                let relative_path = path.strip_prefix(sysroot).unwrap_or(&path);
                path = PathBuf::from("/").join(relative_path);
            }
        }
        (path.to_string_lossy().to_string(), source)
    }

    /// Returns (and caches) a string that uniquely identifies a definition to serve as a key to
    /// the summary cache, which is a key value store. The string will always be the same as
    /// long as the definition does not change its name or location, so it can be used to
    /// transfer information from one compilation to the next, making incremental analysis possible.
    pub fn get_summary_key_for(&mut self, def_id: DefId, tcx: TyCtxt<'tcx>) -> &Rc<str> {
        self.key_cache
            .entry(def_id)
            .or_insert_with(|| utils::summary_key_str(tcx, def_id))
    }

    /// Returns the cached summary corresponding to the function reference.
    /// If the reference has no def_id (and hence no function_id), the entire reference used
    /// as the key, which requires more cache instances and the hard to extract
    /// and unify, duplicated code.
    #[logfn_inputs(TRACE)]
    pub fn get_summary_for_call_site(
        &mut self,
        func_ref: &Rc<FunctionReference>,
        func_args: &Option<Rc<Vec<Rc<FunctionReference>>>>,
        type_args: &Option<Rc<HashMap<Rc<Path>, Ty<'tcx>>>>,
    ) -> &Summary {
        match (func_ref.def_id, func_ref.function_id) {
            // Use the ids as keys if they are available, since they make much better keys.
            (Some(def_id), Some(function_id)) => {
                if func_args.is_some() || type_args.is_some() {
                    let typed_cache_key =
                        CallSiteKey::new(func_args.clone(), type_args.clone(), function_id);
                    // Need the double lookup in order to allow the recursive call to get_summary_for_function_constant.
                    let summary_is_cached = self.call_site_cache.contains_key(&typed_cache_key);
                    return if summary_is_cached {
                        self.call_site_cache.get(&typed_cache_key).unwrap()
                    } else if let Some(summary) = Self::get_persistent_summary_for_db(
                        &self.db,
                        &Self::persistent_call_site_key(func_ref, func_args),
                    ) {
                        self.call_site_cache
                            .entry(typed_cache_key)
                            .or_insert(summary)
                    } else if func_args.is_some() {
                        self.call_site_cache.entry(typed_cache_key).or_default()
                    } else {
                        // can't have self borrowed at this point.
                        let summary = self
                            .get_summary_for_call_site(func_ref, &None, &None)
                            .clone();
                        self.call_site_cache
                            .entry(typed_cache_key)
                            .or_insert(summary)
                    };
                }

                if self.function_id_cache.contains_key(&function_id) {
                    let result = self.function_id_cache.get(&function_id);
                    result.expect("value disappeared from typed_cache")
                } else {
                    if let Some(summary) = self.get_persistent_summary_using_arg_types_if_possible(
                        &func_ref.summary_cache_key,
                        &func_ref.argument_type_key,
                    ) {
                        return self.function_id_cache.entry(function_id).or_insert(summary);
                    }

                    // In this case we default to the summary that is not argument type specific.
                    let db = &self.db;
                    self.def_id_cache.entry(def_id).or_insert_with(|| {
                        let summary =
                            Self::get_persistent_summary_for_db(db, &func_ref.summary_cache_key);
                        summary.unwrap_or_default()
                    })
                }
            }
            // Functions that are included in persisted summaries will not have a def_id (nor a
            // function_id). They were, however, summarized when the summary that included them
            // was created. We look them up in the database. If they are not found there, we use
            // a default summary. Either way, we cache the summary in the appropriate reference cache.
            _ => {
                if self.reference_cache.contains_key(func_ref) {
                    let result = self.reference_cache.get(func_ref);
                    result.expect("value disappeared from typed_reference_cache")
                } else {
                    if let Some(summary) = self.get_persistent_summary_using_arg_types_if_possible(
                        &func_ref.summary_cache_key,
                        &func_ref.argument_type_key,
                    ) {
                        return self
                            .reference_cache
                            .entry(func_ref.clone())
                            .or_insert(summary);
                    }

                    let db = &self.db;
                    self.reference_cache
                        .entry(func_ref.clone())
                        .or_insert_with(|| {
                            let summary = Self::get_persistent_summary_for_db(
                                db,
                                &func_ref.summary_cache_key,
                            );
                            if summary.is_none() {
                                info!(
                                    "Summary store has no entry for {}{}",
                                    &func_ref.summary_cache_key, &func_ref.argument_type_key
                                );
                            };
                            summary.unwrap_or_default()
                        })
                }
            }
        }
    }

    /// Returns a summary from the persistent summary cache, preferentially using the concatenation
    /// of persistent_key with arg_types_key as the cache key and falling back to just the
    /// persistent_key if arg_types_key is None.
    #[logfn(TRACE)]
    pub fn get_persistent_summary_using_arg_types_if_possible(
        &self,
        persistent_key: &str,
        arg_types_key: &str,
    ) -> Option<Summary> {
        if !arg_types_key.is_empty() {
            let mut mangled_key = String::new();
            mangled_key.push_str(persistent_key);
            mangled_key.push_str(arg_types_key);
            Self::get_persistent_summary_for_db(&self.db, mangled_key.as_str())
        } else {
            None
        }
    }

    /// Returns the summary corresponding to the persistent_key in the summary database.
    /// The caller is expected to cache this.
    #[logfn_inputs(TRACE)]
    pub fn get_persistent_summary_for(&self, persistent_key: &str) -> Summary {
        Self::get_persistent_summary_for_db(&self.db, persistent_key).unwrap_or_default()
    }

    pub fn get_persistent_summary_for_call_site(
        &self,
        func_ref: &FunctionReference,
        func_args: &Option<Rc<Vec<Rc<FunctionReference>>>>,
    ) -> Option<Summary> {
        Self::get_persistent_summary_for_db(
            &self.db,
            &Self::persistent_call_site_key(func_ref, func_args),
        )
    }

    /// Helper for get_summary_for and get_persistent_summary_for.
    #[logfn(TRACE)]
    fn get_persistent_summary_for_db(db: &Db, persistent_key: &str) -> Option<Summary> {
        if let Ok(Some(pinned_value)) = db.get(persistent_key.as_bytes()) {
            let bytes = pinned_value.deref();
            Some(deserialize_summary(bytes).unwrap())
        } else {
            None
        }
    }

    /// Sets or updates the typed caches with the call site specialized summary of the
    /// referenced function. Call site specialization involves using the actual generic
    /// arguments supplied by the call site, along with the values of any constant functions
    /// that are supplied as actual arguments.
    #[logfn_inputs(TRACE)]
    pub fn set_summary_for_call_site(
        &mut self,
        func_ref: &Rc<FunctionReference>,
        func_args: &Option<Rc<Vec<Rc<FunctionReference>>>>,
        type_args: &Option<Rc<HashMap<Rc<Path>, Ty<'tcx>>>>,
        summary: Summary,
    ) {
        if let Some(func_id) = func_ref.function_id {
            if !func_ref.argument_type_key.is_empty() || func_args.is_some() {
                let persistent_key = Self::persistent_call_site_key(func_ref, func_args);
                let serialized_summary = bincode::serialize(&summary).unwrap();
                if let Err(error) = self
                    .db
                    .insert(persistent_key.as_bytes(), serialized_summary)
                {
                    println!("unable to set key in summary database: {error:?}");
                }
            }

            // if let Some(def_id) = func_ref.def_id {
            //     if func_args.is_none() && type_args.is_none() {
            //         info!("caching summary for def_id {:?}", def_id);
            //         self.def_id_cache.insert(def_id, summary.clone());
            //     }
            // }
            if func_args.is_some() || type_args.is_some() {
                let typed_cache_key =
                    CallSiteKey::new(func_args.clone(), type_args.clone(), func_id);
                self.call_site_cache.insert(typed_cache_key, summary);
            } else {
                self.function_id_cache.insert(func_id, summary);
            }
        } else {
            //todo: change param to function id
            unreachable!()
        }
    }

    fn persistent_call_site_key(
        func_ref: &FunctionReference,
        func_args: &Option<Rc<Vec<Rc<FunctionReference>>>>,
    ) -> String {
        let mut key = format!(
            "{}{}",
            func_ref.summary_cache_key, func_ref.argument_type_key
        );
        if let Some(func_args) = func_args {
            for argument in func_args.iter() {
                key.push_str("__callback_");
                key.push_str(&argument.summary_cache_key);
                key.push_str(&argument.argument_type_key);
            }
        }
        key
    }

    /// Sets or updates the DefId cache so that from now on def_id maps to the given summary.
    pub fn set_summary_for(
        &mut self,
        def_id: DefId,
        tcx: TyCtxt<'tcx>,
        summary: Summary,
    ) -> Option<Summary> {
        let persistent_key = utils::summary_key_str(tcx, def_id);
        let serialized_summary = bincode::serialize(&summary).unwrap();
        let result = self
            .db
            .insert(persistent_key.as_bytes(), serialized_summary);
        if result.is_err() {
            println!("unable to set key in summary database: {result:?}");
        }
        self.def_id_cache.insert(def_id, summary)
    }
}

#[derive(Serialize)]
pub struct SummariesForLLM {
    // (source path, fully qualified function name, function source, summary)
    entries: Vec<(String, String, String, LLMSummary)>,
}

#[derive(Serialize)]
pub struct FullSummaries {
    // (source path, fully qualified function name, function source, summary)
    entries: Vec<(String, String, String, Summary)>,
}

#[derive(Serialize)]
pub struct ReadableSummaries {
    // (source path, fully qualified function name, function source, summary)
    entries: Vec<(String, String, String, ReadableSummary)>,
}

#[derive(Serialize)]
pub struct ReadableSummary {
    is_computed: bool,
    is_incomplete: bool,
    preconditions: Vec<ReadablePrecondition>,
    assumed_aliases: Vec<ReadableAlias>,
    guarded_aliases: Vec<ReadableGuardedAlias>,
    side_effects: Vec<ReadableEffect>,
    post_condition: Option<String>,
    callback_invocations: Vec<ReadableCallbackInvocation>,
    incomplete_model_state: Vec<ReadableEffect>,
}

#[derive(Serialize)]
pub struct ReadableCallbackInvocation {
    callee: String,
    arguments: Vec<ReadableEffect>,
    arguments_complete: bool,
    pre_state: Vec<ReadableEffect>,
    pre_aliases: Vec<ReadableAlias>,
    pre_guarded_aliases: Vec<ReadableGuardedAlias>,
    guard: String,
    specialized_callee: Option<String>,
    function_constants: Vec<String>,
    is_local: bool,
    carrier_id: Option<u64>,
}

#[derive(Serialize)]
pub struct ReadablePrecondition {
    condition: String,
    message: String,
    provenance: Option<String>,
}

#[derive(Serialize)]
pub struct ReadableAlias {
    left: String,
    right: String,
}

#[derive(Serialize)]
pub struct ReadableGuardedAlias {
    left: String,
    right: String,
    condition: String,
}

#[derive(Serialize)]
pub struct ReadableEffect {
    path: String,
    value: String,
}

#[cfg(test)]
mod tests {
    use super::{
        deserialize_summary, extract_incomplete_model_state, CallbackInvocation,
        PreAliasCallbackInvocation, PreAliasSummary, PreIncompleteModelStateSummary,
        PreLineageCallbackInvocation, PreLineageSummary, Summary, SummaryCache,
    };
    use crate::abstract_value::{self, AbstractValue};
    use crate::constant_domain::FunctionReference;
    use crate::environment::Environment;
    use crate::expression::ExpressionType;
    use crate::known_names::KnownNames;
    use crate::path::Path;
    use rustc_hir::def_id::{DefId, DefIndex};
    use std::env;
    use std::ffi::OsString;
    use std::rc::Rc;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static SUMMARY_STORE_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn incomplete_callback_cleanup_rekeys_only_captured_model_state() {
        let captured_path = Path::new_model_field(Path::new_local(1, 0), Rc::from("write_held"));
        let unrelated_path = Path::new_model_field(Path::new_local(2, 0), Rc::from("write_held"));
        let boundary_path = Path::new_model_field(Path::new_parameter(1), Rc::from("write_held"));
        let mut environment = Environment::default();
        environment.strong_update_value_at(captured_path.clone(), Rc::new(0_u128.into()));
        environment.strong_update_value_at(unrelated_path, Rc::new(0_u128.into()));
        let invocation = CallbackInvocation {
            callee: Path::new_local(3, 0),
            arguments: Vec::new(),
            arguments_complete: false,
            pre_state: vec![(boundary_path.clone(), Rc::new(1_u128.into()))],
            pre_aliases: Vec::new(),
            pre_guarded_aliases: Vec::new(),
            guard: Rc::new(abstract_value::TRUE),
            specialized_callee: None,
            function_constants: Vec::new(),
            is_local: false,
            carrier_id: None,
            state_rekeys: vec![(
                captured_path,
                AbstractValue::make_typed_unknown(ExpressionType::U128, boundary_path.clone()),
            )],
        };

        let expected_state = vec![(boundary_path, Rc::new(0_u128.into()))];
        assert_eq!(
            extract_incomplete_model_state(&[&environment], &[invocation.clone()]),
            expected_state
        );

        let summary = Summary {
            is_computed: true,
            callback_invocations: vec![invocation],
            incomplete_model_state: expected_state.clone(),
            ..Summary::default()
        };
        let bytes = bincode::serialize(&summary).unwrap();
        let deserialized = deserialize_summary(&bytes).unwrap();
        assert_eq!(deserialized.incomplete_model_state, expected_state);
        assert!(deserialized.callback_invocations[0].state_rekeys.is_empty());
    }

    #[test]
    fn pre_incomplete_model_state_summary_remains_deserializable() {
        let old_summary = PreIncompleteModelStateSummary {
            is_computed: true,
            is_incomplete: true,
            preconditions: Vec::new(),
            assumed_aliases: Vec::new(),
            guarded_aliases: Vec::new(),
            side_effects: Vec::new(),
            post_condition: None,
            callback_invocations: Vec::new(),
        };

        let bytes = bincode::serialize(&old_summary).unwrap();
        let summary = deserialize_summary(&bytes).unwrap();

        assert!(summary.incomplete_model_state.is_empty());
    }

    #[test]
    fn pre_lineage_callback_summary_remains_deserializable() {
        let callee = Path::new_parameter(1);
        let old_summary = PreLineageSummary {
            is_computed: true,
            is_incomplete: false,
            preconditions: Vec::new(),
            assumed_aliases: Vec::new(),
            guarded_aliases: Vec::new(),
            side_effects: Vec::new(),
            post_condition: None,
            callback_invocations: vec![PreLineageCallbackInvocation {
                callee: callee.clone(),
                arguments: Vec::new(),
                arguments_complete: true,
                pre_state: Vec::new(),
                pre_aliases: Vec::new(),
                pre_guarded_aliases: Vec::new(),
                guard: Rc::new(abstract_value::TRUE),
                specialized_callee: None,
                function_constants: Vec::new(),
                is_local: false,
            }],
        };

        let bytes = bincode::serialize(&old_summary).unwrap();
        let summary = deserialize_summary(&bytes).unwrap();
        let invocation = summary.callback_invocations.first().unwrap();

        assert_eq!(invocation.callee, callee);
        assert_eq!(invocation.carrier_id, None);
    }

    #[test]
    fn readable_callback_invocations_are_structured_json() {
        let invocation = CallbackInvocation {
            callee: Path::new_parameter(2),
            arguments: vec![(Path::new_parameter(1), Rc::new(42_u128.into()))],
            arguments_complete: true,
            pre_state: Vec::new(),
            pre_aliases: Vec::new(),
            pre_guarded_aliases: Vec::new(),
            guard: Rc::new(abstract_value::TRUE),
            specialized_callee: None,
            function_constants: Vec::new(),
            is_local: false,
            carrier_id: Some(7),
            state_rekeys: vec![(Path::new_parameter(3), Rc::new(1_u128.into()))],
        };
        let readable = super::ReadableSummary::from(&Summary {
            callback_invocations: vec![invocation],
            ..Summary::default()
        });

        let value = serde_json::to_value(readable).unwrap();
        let callback = &value["callback_invocations"][0];
        assert!(callback.is_object());
        assert_eq!(callback["callee"], "param_2");
        assert_eq!(callback["arguments"][0]["path"], "param_1");
        assert_eq!(callback["carrier_id"], 7);
        assert!(callback.get("state_rekeys").is_none());
    }

    #[test]
    fn pre_alias_callback_summary_remains_deserializable() {
        let callee = Path::new_parameter(1);
        let old_summary = PreAliasSummary {
            is_computed: true,
            is_incomplete: false,
            preconditions: Vec::new(),
            assumed_aliases: Vec::new(),
            guarded_aliases: Vec::new(),
            side_effects: Vec::new(),
            post_condition: None,
            callback_invocations: vec![PreAliasCallbackInvocation {
                callee: callee.clone(),
                arguments: Vec::new(),
                arguments_complete: true,
                pre_state: Vec::new(),
                guard: Rc::new(abstract_value::TRUE),
                specialized_callee: None,
                function_constants: Vec::new(),
                is_local: false,
            }],
        };

        let bytes = bincode::serialize(&old_summary).unwrap();
        let summary = deserialize_summary(&bytes).unwrap();
        let invocation = summary.callback_invocations.first().unwrap();

        assert_eq!(invocation.callee, callee);
        assert!(invocation.pre_aliases.is_empty());
        assert!(invocation.pre_guarded_aliases.is_empty());
    }

    struct SummaryStoreEnvironment {
        start_fresh: Option<OsString>,
        share_persistent_store: Option<OsString>,
    }

    impl SummaryStoreEnvironment {
        fn configure_shared_store() -> Self {
            let environment = Self {
                start_fresh: env::var_os("MIRAI_START_FRESH"),
                share_persistent_store: env::var_os("MIRAI_SHARE_PERSISTENT_STORE"),
            };
            env::remove_var("MIRAI_START_FRESH");
            env::set_var("MIRAI_SHARE_PERSISTENT_STORE", "true");
            environment
        }
    }

    impl Drop for SummaryStoreEnvironment {
        fn drop(&mut self) {
            match &self.start_fresh {
                Some(value) => env::set_var("MIRAI_START_FRESH", value),
                None => env::remove_var("MIRAI_START_FRESH"),
            }
            match &self.share_persistent_store {
                Some(value) => env::set_var("MIRAI_SHARE_PERSISTENT_STORE", value),
                None => env::remove_var("MIRAI_SHARE_PERSISTENT_STORE"),
            }
        }
    }

    #[test]
    fn shared_store_seeds_embedded_standard_summaries() {
        let _lock = SUMMARY_STORE_ENVIRONMENT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let store_directory = TempDir::new().expect("failed to create temporary summary store");
        let _environment = SummaryStoreEnvironment::configure_shared_store();

        let cache = SummaryCache::new(
            store_directory
                .path()
                .to_str()
                .expect("temporary path should be valid UTF-8")
                .to_owned(),
        );

        let missing_keys: Vec<_> = ["core.result.unwrap_failed", "core.option.unwrap_failed"]
            .into_iter()
            .filter(|key| !cache.get_persistent_summary_for(key).is_computed)
            .collect();
        assert!(
            missing_keys.is_empty(),
            "shared store did not load {}",
            missing_keys.join(", ")
        );
    }

    #[test]
    fn adapter_closure_specialization_is_replay_resolvable() {
        let _lock = SUMMARY_STORE_ENVIRONMENT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let store_directory = TempDir::new().expect("failed to create temporary summary store");
        let _environment = SummaryStoreEnvironment::configure_shared_store();
        let user_callback_ref = Rc::new(FunctionReference {
            def_id: Some(DefId::local(DefIndex::from_u32(1))),
            function_id: Some(1),
            generic_arguments: vec![],
            known_name: KnownNames::None,
            summary_cache_key: "fixture.main.closure".into(),
            argument_type_key: "_user_callback".into(),
        });
        let adapter_ref = Rc::new(FunctionReference {
            def_id: Some(DefId::local(DefIndex::from_u32(2))),
            function_id: Some(2),
            generic_arguments: vec![],
            known_name: KnownNames::None,
            summary_cache_key: "fixture.descriptor_table_mut.closure".into(),
            argument_type_key: "_adapter".into(),
        });
        let summary = Summary {
            is_computed: true,
            ..Summary::default()
        };
        let store_path = store_directory
            .path()
            .to_str()
            .expect("temporary path should be valid UTF-8")
            .to_owned();

        {
            let mut cache = SummaryCache::new(store_path.clone());
            let write_arguments = Some(Rc::new(vec![
                adapter_ref.clone(),
                user_callback_ref.clone(),
            ]));
            cache.set_summary_for_call_site(&adapter_ref, &write_arguments, &None, summary);
        }

        let mut reopened_cache = SummaryCache::new(store_path);
        let replay_arguments = Some(Rc::new(vec![adapter_ref.clone(), user_callback_ref]));
        assert!(
            reopened_cache
                .get_summary_for_call_site(&adapter_ref, &replay_arguments, &None)
                .is_computed,
            "adapter closure summary is not reachable through the full replay signature"
        );
    }
}

impl SummariesForLLM {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self).unwrap()
    }
}

impl FullSummaries {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self).unwrap()
    }
}

impl ReadableSummaries {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self).unwrap()
    }
}

impl From<&Summary> for ReadableSummary {
    fn from(summary: &Summary) -> Self {
        Self {
            is_computed: summary.is_computed,
            is_incomplete: summary.is_incomplete,
            preconditions: summary
                .preconditions
                .iter()
                .map(|precondition| ReadablePrecondition {
                    condition: format!("{:?}", precondition.condition),
                    message: precondition.message.to_string(),
                    provenance: precondition.provenance.as_deref().map(str::to_owned),
                })
                .collect(),
            assumed_aliases: summary
                .assumed_aliases
                .iter()
                .map(|(left, right)| ReadableAlias {
                    left: format!("{left:?}"),
                    right: format!("{right:?}"),
                })
                .collect(),
            guarded_aliases: summary
                .guarded_aliases
                .iter()
                .map(|(left, right, condition)| ReadableGuardedAlias {
                    left: format!("{left:?}"),
                    right: format!("{right:?}"),
                    condition: format!("{condition:?}"),
                })
                .collect(),
            side_effects: readable_effects(&summary.side_effects),
            post_condition: summary
                .post_condition
                .as_ref()
                .map(|condition| format!("{condition:?}")),
            callback_invocations: summary
                .callback_invocations
                .iter()
                .map(ReadableCallbackInvocation::from)
                .collect(),
            incomplete_model_state: readable_effects(&summary.incomplete_model_state),
        }
    }
}

impl From<&CallbackInvocation> for ReadableCallbackInvocation {
    fn from(invocation: &CallbackInvocation) -> Self {
        Self {
            callee: format!("{:?}", invocation.callee),
            arguments: readable_effects(&invocation.arguments),
            arguments_complete: invocation.arguments_complete,
            pre_state: readable_effects(&invocation.pre_state),
            pre_aliases: invocation
                .pre_aliases
                .iter()
                .map(|(left, right)| ReadableAlias {
                    left: format!("{left:?}"),
                    right: format!("{right:?}"),
                })
                .collect(),
            pre_guarded_aliases: invocation
                .pre_guarded_aliases
                .iter()
                .map(|(left, right, condition)| ReadableGuardedAlias {
                    left: format!("{left:?}"),
                    right: format!("{right:?}"),
                    condition: format!("{condition:?}"),
                })
                .collect(),
            guard: format!("{:?}", invocation.guard),
            specialized_callee: invocation
                .specialized_callee
                .as_ref()
                .map(|callee| format!("{callee:?}")),
            function_constants: invocation
                .function_constants
                .iter()
                .map(|function| format!("{function:?}"))
                .collect(),
            is_local: invocation.is_local,
            carrier_id: invocation.carrier_id,
        }
    }
}

fn readable_effects(effects: &[(Rc<Path>, Rc<AbstractValue>)]) -> Vec<ReadableEffect> {
    effects
        .iter()
        .map(|(path, value)| ReadableEffect {
            path: format!("{path:?}"),
            value: format!("{value:?}"),
        })
        .collect()
}

#[derive(Serialize)]
pub struct LLMSummary {
    // Conditions that should hold prior to the call.
    // Callers should substitute parameter values with argument values and simplify the results
    // under the current path condition. Any values that do not simplify to true will require the
    // caller to either generate an error message or to add a precondition to its own summary that
    // will be sufficient to ensure that all the preconditions in this summary are met.
    // The string value bundled with the condition is the message that details what would go
    // wrong at runtime if the precondition is not satisfied by the caller.
    //pub preconditions: Vec<Precondition>,

    // Modifications the function makes to mutable state external to the function.
    // Every path will be rooted in a static or in a mutable parameter.
    // No two paths in this collection will lead to the same place in memory.
    // Callers should substitute parameter values with argument values and simplify the results
    // under the current path condition. They should then update their current state to reflect the
    // side effects of the call.
    //pub side_effects: Vec<(Rc<Path>, Rc<AbstractValue>)>,

    // A condition that should hold after a call that completes normally.
    // Callers should substitute parameter values with argument values and simplify the results
    // under the current path condition.
    // The resulting value should be conjoined to the current path condition.
    //pub post_condition: Option<Rc<AbstractValue>>,

    // The set of function calls made by this function. The first element is the source snippet of
    // the call and the second is the fully qualified name of the function being called.
    calls: Vec<(String, String)>,
}

impl LLMSummary {
    pub fn from_summary(_summary: &Summary, calls: Vec<(String, String)>) -> LLMSummary {
        LLMSummary {
            // preconditions: vec![],
            // side_effects: vec![],
            // post_condition: vec![],
            calls,
        }
    }
}
