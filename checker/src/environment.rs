// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use crate::abstract_value;
use crate::abstract_value::AbstractValue;
use crate::abstract_value::AbstractValueTrait;
use crate::expression::{Expression, ExpressionType};
use crate::path::{Path, PathEnum, PathRefinement, PathRoot, PathSelector};

use crate::body_visitor::BodyVisitor;
use crate::constant_domain::ConstantDomain;
use log_derive::{logfn, logfn_inputs};
use rpds::HashTrieMap;
use rustc_middle::mir::BasicBlock;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter, Result};
use std::rc::Rc;

#[derive(Clone, Eq, PartialEq)]
pub struct Environment {
    /// The disjunction of all the exit conditions from the predecessors of this block.
    pub entry_condition: Rc<AbstractValue>,
    /// Alias relationships that hold on every path reaching this environment.
    pub assumed_aliases: HashSet<(Rc<Path>, Rc<Path>)>,
    /// Alias relationships guarded by conditions over boundary-visible values.
    pub guarded_aliases: HashMap<(Rc<Path>, Rc<Path>), Rc<AbstractValue>>,
    /// The conditions that guard exit from this block to successor blocks
    pub exit_conditions: HashTrieMap<BasicBlock, Rc<AbstractValue>>,
    /// Does not include any entries where the value is abstract_value::Bottom
    pub value_map: HashTrieMap<Rc<Path>, Rc<AbstractValue>>,
}

/// Default
impl Default for Environment {
    #[logfn_inputs(TRACE)]
    fn default() -> Environment {
        Environment {
            entry_condition: Rc::new(abstract_value::TRUE),
            assumed_aliases: HashSet::new(),
            guarded_aliases: HashMap::new(),
            exit_conditions: HashTrieMap::default(),
            value_map: HashTrieMap::default(),
        }
    }
}

impl Debug for Environment {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_map().entries(self.value_map.iter()).finish()
    }
}

/// Methods
impl Environment {
    /// True when `path` has not been reassigned in this environment, so `old(path)` and the
    /// current `path` denote the same pointer. This holds when there is no binding (still the
    /// entry value), when it binds to the dummy self-referential `old(path)` (the same
    /// loop-invariance oracle used in `Path::canonicalize`), or when it binds to a `Reference`,
    /// i.e. the parameter's own `&p`. A later reassignment lands on a distinct current value that
    /// never keys the `&p` model field, so a reassigned pointer cannot be collapsed here.
    fn parameter_is_invariant(&self, path: &Rc<Path>) -> bool {
        self.value_at(path).is_none_or(|value| {
            matches!(
                &value.expression,
                Expression::InitialParameterValue {
                    path: initial_path,
                    ..
                } if initial_path == path
            ) || matches!(
                &value.expression,
                // Replay stores the current Arc root as &p; reassignment replaces this binding.
                Expression::Reference(_)
            )
        })
    }

    fn strip_invariant_old_from_pointer_value(
        &self,
        value: &Rc<AbstractValue>,
    ) -> Rc<AbstractValue> {
        let expression = match &value.expression {
            Expression::InitialParameterValue { path, var_type }
                if self.parameter_is_invariant(path) =>
            {
                if *var_type == ExpressionType::U128 {
                    return AbstractValue::make_reference(path.clone());
                }
                return AbstractValue::make_typed_unknown(*var_type, path.clone());
            }
            Expression::BitAnd { left, right } => Expression::BitAnd {
                left: self.strip_invariant_old_from_pointer_value(left),
                right: self.strip_invariant_old_from_pointer_value(right),
            },
            Expression::Cast {
                operand,
                target_type,
            } => Expression::Cast {
                operand: self.strip_invariant_old_from_pointer_value(operand),
                target_type: *target_type,
            },
            Expression::Offset { left, right } => Expression::Offset {
                left: self.strip_invariant_old_from_pointer_value(left),
                right: self.strip_invariant_old_from_pointer_value(right),
            },
            Expression::Rem { left, right } => Expression::Rem {
                left: self.strip_invariant_old_from_pointer_value(left),
                right: self.strip_invariant_old_from_pointer_value(right),
            },
            Expression::Transmute {
                operand,
                target_type,
            } => Expression::Transmute {
                operand: self.strip_invariant_old_from_pointer_value(operand),
                target_type: *target_type,
            },
            _ => return value.clone(),
        };
        AbstractValue::make_from(expression, value.expression_size)
    }

    fn strip_invariant_old_from_pointer_root(&self, path: Rc<Path>) -> Rc<Path> {
        match &path.value {
            PathEnum::Computed { value } => {
                Path::new_computed(self.strip_invariant_old_from_pointer_value(value))
            }
            PathEnum::Offset { value } => {
                Path::get_as_path(self.strip_invariant_old_from_pointer_value(value))
            }
            PathEnum::QualifiedPath {
                qualifier,
                selector,
                ..
            } => Path::new_qualified(
                self.strip_invariant_old_from_pointer_root(qualifier.clone()),
                selector.clone(),
            ),
            _ => path,
        }
    }

    /// Records an alias relationship that holds throughout this environment.
    pub fn assume_alias(&mut self, alias: Rc<Path>, source: Rc<Path>) {
        let relationship = (alias, source);
        self.guarded_aliases.remove(&relationship);
        self.assumed_aliases.insert(relationship);
    }

    /// Returns true if the two paths are identical after applying must-alias relationships.
    pub fn paths_must_alias(&self, left: &Rc<Path>, right: &Rc<Path>) -> bool {
        fn canonicalize(env: &Environment, path: Rc<Path>) -> Rc<Path> {
            let mut result = path;
            let mut visited = HashSet::new();
            let mut aliases: Vec<_> = env
                .assumed_aliases
                .iter()
                .filter(|(alias, source)| !source.is_rooted_by(alias))
                .collect();
            aliases.sort_by(|(left_alias, left_source), (right_alias, right_source)| {
                left_alias
                    .cmp(right_alias)
                    .then_with(|| left_source.cmp(right_source))
            });
            while visited.insert(result.clone()) {
                let mut best: Option<(&Rc<Path>, &Rc<Path>)> = None;
                for (alias, source) in &aliases {
                    if result != *alias && !result.is_rooted_by(alias) {
                        continue;
                    }
                    if best
                        .as_ref()
                        .is_none_or(|(current, _)| alias.is_rooted_by(current))
                    {
                        best = Some((alias, source));
                    }
                }
                let Some((alias, source)) = best else {
                    break;
                };
                result = result.replace_root(alias, source.clone());
            }
            result
        }

        canonicalize(self, left.clone()) == canonicalize(self, right.clone())
    }

    /// Records an alias relationship that holds when condition is true.
    pub fn assume_alias_if(
        &mut self,
        alias: Rc<Path>,
        source: Rc<Path>,
        condition: Rc<AbstractValue>,
    ) {
        let relationship = (alias, source);
        if self.assumed_aliases.contains(&relationship) {
            return;
        }
        match condition.as_bool_if_known() {
            Some(true) => self.assume_alias(relationship.0, relationship.1),
            Some(false) => {}
            None => {
                self.guarded_aliases
                    .entry(relationship)
                    .and_modify(|existing| *existing = existing.or(condition.clone()))
                    .or_insert(condition);
            }
        }
    }

    /// Carries must-alias relationships along when an aggregate is copied or moved.
    #[logfn_inputs(TRACE)]
    pub fn copy_aliases_rooted_by(
        &mut self,
        target: &Rc<Path>,
        source: &Rc<Path>,
        move_elements: bool,
    ) {
        if target == source {
            return;
        }
        let assumed_aliases: Vec<_> = self
            .assumed_aliases
            .iter()
            .filter_map(|(alias, alias_source)| {
                let copied_alias = (alias == source || alias.is_rooted_by(source))
                    .then(|| alias.replace_root(source, target.clone()))
                    .unwrap_or_else(|| alias.clone());
                let copied_source = (alias_source == source || alias_source.is_rooted_by(source))
                    .then(|| alias_source.replace_root(source, target.clone()))
                    .unwrap_or_else(|| alias_source.clone());
                (copied_alias != copied_source
                    && (copied_alias != *alias || copied_source != *alias_source))
                    .then_some((copied_alias, copied_source))
            })
            .collect();
        self.assumed_aliases.extend(assumed_aliases);
        if move_elements {
            self.assumed_aliases.retain(|(alias, alias_source)| {
                !(alias == source
                    || alias.is_rooted_by(source)
                    || alias_source == source
                    || alias_source.is_rooted_by(source))
            });
        }

        let guarded_aliases: Vec<_> = self
            .guarded_aliases
            .iter()
            .filter_map(|((alias, alias_source), condition)| {
                let copied_alias = (alias == source || alias.is_rooted_by(source))
                    .then(|| alias.replace_root(source, target.clone()))
                    .unwrap_or_else(|| alias.clone());
                let copied_source = (alias_source == source || alias_source.is_rooted_by(source))
                    .then(|| alias_source.replace_root(source, target.clone()))
                    .unwrap_or_else(|| alias_source.clone());
                if copied_alias == copied_source
                    || (copied_alias == *alias && copied_source == *alias_source)
                {
                    return None;
                }
                let copied_condition = condition.replace_embedded_path_root(source, target.clone());
                Some(((copied_alias, copied_source), copied_condition))
            })
            .collect();
        for ((alias, alias_source), condition) in guarded_aliases {
            self.assume_alias_if(alias, alias_source, condition);
        }
        if move_elements {
            self.guarded_aliases.retain(|(alias, alias_source), _| {
                !(alias == source
                    || alias.is_rooted_by(source)
                    || alias_source == source
                    || alias_source.is_rooted_by(source))
            });
        }
    }

    /// Rewrites aliases in the qualifier of a model-field path to their canonical sources.
    pub fn canonicalize_model_field_path(&self, path: Rc<Path>) -> Rc<Path> {
        let PathEnum::QualifiedPath { selector, .. } = &path.value else {
            return path;
        };
        if !matches!(
            selector.as_ref(),
            PathSelector::ModelField(_) | PathSelector::TagField
        ) {
            return path;
        }
        let path = path.canonicalize_reference_projections(self);

        let mut result = path;
        let mut visited = HashSet::new();
        let mut aliases: Vec<_> = self
            .assumed_aliases
            .iter()
            .filter(|(alias, source)| !source.is_rooted_by(alias))
            .collect();
        aliases.sort_by(|(left_alias, left_source), (right_alias, right_source)| {
            left_alias
                .cmp(right_alias)
                .then_with(|| left_source.cmp(right_source))
        });
        while visited.insert(result.clone()) {
            let mut best: Option<(&Rc<Path>, &Rc<Path>)> = None;
            for (alias, source) in &aliases {
                if result != *alias && !result.is_rooted_by(alias) {
                    continue;
                }
                if best
                    .as_ref()
                    .is_none_or(|(current, _)| alias.is_rooted_by(current))
                {
                    best = Some((alias, source));
                }
            }
            let Some((alias, source)) = best else {
                break;
            };
            result = result.replace_root(alias, source.clone());
        }
        result
    }

    /// Looks up a model field through unconditional and guarded alias prefixes.
    #[logfn_inputs(TRACE)]
    pub fn value_at_aliased_model_field(
        &self,
        path: &Rc<Path>,
        default: Rc<AbstractValue>,
    ) -> Option<Rc<AbstractValue>> {
        let canonical_path = self.canonicalize_model_field_path(path.clone());
        // Lookup-on-miss fallback: a serialized callback pre_state writes the model field through
        // the current pointer root, while the replayed precondition reads it through `old(ptr)`.
        // Only when the miss is on an invariant pointer do we retry against the current-root key,
        // keeping the write side untouched to avoid consuming the seeded obligation prematurely.
        let direct_value = self.value_map.get(&canonical_path).cloned().or_else(|| {
            let current_pointer_path =
                self.strip_invariant_old_from_pointer_root(canonical_path.clone());
            (current_pointer_path != canonical_path)
                .then(|| self.value_map.get(&current_pointer_path).cloned())
                .flatten()
        });
        let mut value = direct_value.clone().unwrap_or(default);
        let mut found = direct_value.is_some();

        let mut aliases: Vec<_> = self.guarded_aliases.iter().collect();
        aliases.sort_by(
            |((left_alias, left_source), _), ((right_alias, right_source), _)| {
                left_alias
                    .cmp(right_alias)
                    .then_with(|| left_source.cmp(right_source))
            },
        );
        for ((alias, source), condition) in aliases {
            if path != alias && !path.is_rooted_by(alias) {
                continue;
            }
            let source_path = path.replace_root(alias, source.clone());
            let source_path = self.canonicalize_model_field_path(source_path);
            let Some(source_value) = self.value_map.get(&source_path) else {
                continue;
            };
            value = condition.conditional_expression(source_value.clone(), value);
            found = true;
        }
        found.then_some(value)
    }

    /// Returns a reference to the value associated with the given path, if there is one.
    #[logfn_inputs(TRACE)]
    pub fn value_at(&self, path: &Rc<Path>) -> Option<&Rc<AbstractValue>> {
        self.value_map.get(path)
    }

    /// Looks up a model field while accounting for computed indices that may alias.
    pub fn value_at_computed_index_model_field(
        &self,
        path: &Rc<Path>,
        default: Rc<AbstractValue>,
    ) -> Option<Rc<AbstractValue>> {
        if let Some(value) = self.value_map.get(path) {
            return Some(value.clone());
        }

        let (source_collection, source_field) = Self::computed_index_model_field_parts(path)?;
        let candidates = self.value_map.iter().filter_map(|(candidate_path, value)| {
            let (candidate_collection, candidate_field) =
                Self::computed_index_model_field_parts(candidate_path)?;
            (candidate_collection == source_collection && candidate_field == source_field)
                .then(|| (candidate_path, value))
        });

        let mut proven_value: Option<Rc<AbstractValue>> = None;
        let mut conditional_values = Vec::new();
        for (candidate_path, candidate_value) in candidates {
            let paths_are_equal = candidate_path.equals(path);
            if self.entry_condition.implies(&paths_are_equal) {
                proven_value = Some(match proven_value {
                    Some(value) => value.join(candidate_value.clone()),
                    None => candidate_value.clone(),
                });
            } else if !self.entry_condition.implies(&paths_are_equal.logical_not()) {
                conditional_values.push((paths_are_equal, candidate_value.clone()));
            }
        }

        let found_candidate = proven_value.is_some() || !conditional_values.is_empty();
        let mut value = proven_value.unwrap_or(default);
        for (condition, candidate_value) in conditional_values {
            let aliased_value = value.join(candidate_value);
            value = condition.conditional_expression(aliased_value, value);
        }
        found_candidate.then_some(value)
    }

    fn computed_index_model_field_parts(path: &Rc<Path>) -> Option<(&Rc<Path>, &Rc<str>)> {
        let PathEnum::QualifiedPath {
            qualifier,
            selector,
            ..
        } = &path.value
        else {
            return None;
        };
        let PathSelector::ModelField(field) = selector.as_ref() else {
            return None;
        };
        let PathEnum::QualifiedPath {
            qualifier: collection,
            selector: index,
            ..
        } = &qualifier.value
        else {
            return None;
        };
        matches!(index.as_ref(), PathSelector::Index(_)).then_some((collection, field))
    }

    /// Updates the path to value map so that the given path now points to the given value.
    #[logfn_inputs(TRACE)]
    pub fn strong_update_value_at(&mut self, path: Rc<Path>, value: Rc<AbstractValue>) {
        self.value_map.insert_mut(path, value);
    }

    /// Update any paths that might alias path to now point to a weaker abstract value that
    /// includes all of the concrete values that value might be at runtime.
    #[logfn_inputs(TRACE)]
    pub fn weakly_update_aliases(
        &mut self,
        path: Rc<Path>,
        value: Rc<AbstractValue>,
        path_condition: Rc<AbstractValue>,
        body_visitor: &mut BodyVisitor,
    ) {
        if let Some((condition, true_path, false_path)) = self.try_to_split(&path) {
            // The value path contains an abstract value that was constructed with a conditional.
            // In this case, we split the path into two and perform conditional weak updates on both.
            // Rather than do it here, we recurse until there are no more conditionals.
            self.weakly_update_aliases(
                true_path,
                value.clone(),
                path_condition.and(condition.clone()),
                body_visitor,
            );
            self.weakly_update_aliases(
                false_path,
                value,
                path_condition.and(condition.logical_not()),
                body_visitor,
            );
            return;
        }
        // Incorporate path_condition into value.
        let value = if path_condition.as_bool_if_known().is_none() {
            // If the path condition is true, the value of path will be updated with value, otherwise use:
            let old_value = if let Some(v) = self.value_map.get(&path) {
                v.clone()
            } else {
                AbstractValue::make_typed_unknown(value.expression.infer_type(), path.clone())
            };
            // Combine old with new to get a weakened value
            let weak_value = path_condition.conditional_expression(value, old_value);
            // Do a strong update of path using a weakened value
            self.value_map.insert_mut(path.clone(), weak_value.clone());
            weak_value
        } else {
            value
        };
        // Now look for potential aliases of path that also need updating
        if let PathEnum::QualifiedPath {
            qualifier,
            selector,
            ..
        } = &path.value
        {
            match selector.as_ref() {
                PathSelector::ConstantIndex {
                    offset, from_end, ..
                } => {
                    let mut index = Rc::new((*offset as u128).into());
                    if *from_end {
                        let length_path = Path::new_length(qualifier.clone());
                        let length_val =
                            AbstractValue::make_typed_unknown(ExpressionType::Usize, length_path);
                        index = length_val.subtract(index);
                    };
                    self.weaken_potential_aliased_index_paths(
                        &path,
                        &value,
                        qualifier,
                        &index,
                        body_visitor,
                    )
                }
                PathSelector::Deref => {
                    // we are assigning value to *qualifier and there may be another path *q where
                    // qualifier and q may be the same path at runtime.
                    let value_map = self.value_map.clone();
                    for (p, v) in value_map.iter() {
                        if p.eq(&path) {
                            continue;
                        }
                        if let PathEnum::QualifiedPath {
                            qualifier: qs,
                            selector: s,
                            ..
                        } = &p.value
                        {
                            if **s == PathSelector::Deref {
                                let paths_are_equal = qualifier.equals(qs);
                                match paths_are_equal.as_bool_if_known() {
                                    Some(true) => {
                                        // p is known to be an alias of path, so just update it
                                        self.strong_update_value_at(p.clone(), value.clone());
                                    }
                                    Some(false) => {
                                        // p is known not to be an alias of path
                                        continue;
                                    }
                                    None => {
                                        // p might be an alias of, so weaken its value by making it
                                        // conditional on path_are_equal
                                        let conditional_value = paths_are_equal
                                            .conditional_expression(value.clone(), v.clone());
                                        self.strong_update_value_at(p.clone(), conditional_value);
                                    }
                                }
                            }
                        }
                        if let PathEnum::HeapBlock { .. } = &p.value {
                            let paths_are_equal = qualifier.equals(p);
                            match paths_are_equal.as_bool_if_known() {
                                Some(true) => {
                                    // p is known to be an alias of path, so just update it
                                    self.strong_update_value_at(p.clone(), value.clone());
                                }
                                Some(false) => {
                                    // p is known not to be an alias of path
                                    continue;
                                }
                                None => {
                                    // p might be an alias of, so weaken its value by making it
                                    // conditional on path_are_equal
                                    let conditional_value = paths_are_equal
                                        .conditional_expression(value.clone(), v.clone());
                                    self.strong_update_value_at(p.clone(), conditional_value);
                                }
                            }
                        }
                    }
                }
                PathSelector::Index(index) => self.weaken_potential_aliased_index_paths(
                    &path,
                    &value,
                    qualifier,
                    index,
                    body_visitor,
                ),
                PathSelector::Slice(count) => {
                    // We are assigning value to every element of the slice qualifier[0..count]
                    // There may already be paths for individual elements of the slice.
                    let value_map = self.value_map.clone();
                    for (p, v) in value_map.iter() {
                        if p.eq(&path) {
                            continue;
                        }
                        if let PathEnum::QualifiedPath {
                            qualifier: paq,
                            selector: pas,
                            ..
                        } = &p.value
                        {
                            if paq.ne(qualifier) {
                                // p is not an alias because its qualifier does not match
                                continue;
                            }
                            match pas.as_ref() {
                                PathSelector::ConstantIndex { .. }
                                | PathSelector::ConstantSlice { .. } => {
                                    unreachable!("path {:?} p {:?} v {:?}", path, p, v);
                                }
                                PathSelector::Index(index) => {
                                    // paq[index] might alias an element in qualifier[0..count]
                                    let index_is_in_range = index.less_than(count.clone());
                                    match index_is_in_range.as_bool_if_known() {
                                        Some(true) => {
                                            // p is an alias for sure, so just update it
                                            self.strong_update_value_at(p.clone(), value.clone());
                                        }
                                        Some(false) => {
                                            // p is known not to be an alias
                                            continue;
                                        }
                                        None => {
                                            // p might be an alias, so weaken its value by joining it
                                            // with the slice initializer.
                                            let weakened_value = v.join(value.clone());
                                            // If index is not in range, use the strong value
                                            let guarded_weakened_value = index_is_in_range
                                                .conditional_expression(weakened_value, v.clone());
                                            self.strong_update_value_at(
                                                p.clone(),
                                                guarded_weakened_value,
                                            );
                                        }
                                    }
                                }
                                PathSelector::Slice(c) => {
                                    // The elements of paq[0..c] alias elements of qualifier[0..count]
                                    // If c <= count, then all of the elements of paq[0..c] will be updated with value.
                                    // If c > count, then some of elements will still have value v.
                                    let aliased_slice_is_smaller_or_equal =
                                        c.less_or_equal(count.clone());
                                    let weakened_value = v.join(value.clone());
                                    let guarded_weakened_value = aliased_slice_is_smaller_or_equal
                                        .conditional_expression(value.clone(), weakened_value);
                                    self.strong_update_value_at(p.clone(), guarded_weakened_value);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                PathSelector::ConstantSlice { .. } => {
                    // empty slice, or too large slice, do nothing
                }
                _ => {
                    // we are assigning value to qualifier.selector and there may be another path q.selector where
                    // qualifier and q may be the same path at runtime (and hence should be treated
                    // as a potential alias).
                    let value_map = self.value_map.clone();
                    for (p, v) in value_map.iter() {
                        if p.eq(&path) {
                            continue;
                        }
                        if let PathEnum::QualifiedPath {
                            qualifier: qs,
                            selector: s,
                            ..
                        } = &p.value
                        {
                            if s.eq(selector) {
                                let paths_are_equal = qualifier.equals(qs);
                                match paths_are_equal.as_bool_if_known() {
                                    Some(true) => {
                                        // p is known to be an alias of path, so just update it
                                        self.strong_update_value_at(p.clone(), value.clone());
                                    }
                                    Some(false) => {
                                        // p is known not to be an alias of path
                                        continue;
                                    }
                                    None => {
                                        // p might be an alias of, so weaken its value by making it
                                        // conditional on path_are_equal
                                        let conditional_value = paths_are_equal
                                            .conditional_expression(value.clone(), v.clone());
                                        self.strong_update_value_at(p.clone(), conditional_value);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// We are assigning value to qualifier[index] and there may be other (p[i], v)
    /// pairs in the environment where the runtime location of p[i] might turn out to
    /// be the same as the runtime location of qualifier[index] (i.e. path).
    /// We have to model this uncertainty by weakening v to include the set of
    /// concrete values represented by value.
    fn weaken_potential_aliased_index_paths(
        &mut self,
        path: &Rc<Path>,
        value: &Rc<AbstractValue>,
        qualifier: &Rc<Path>,
        index: &Rc<AbstractValue>,
        body_visitor: &mut BodyVisitor,
    ) {
        let value_map = self.value_map.clone();
        for (p, v) in value_map.iter() {
            check_for_early_return!(body_visitor);
            if p.eq(path) {
                continue;
            }
            if let PathEnum::QualifiedPath {
                qualifier: paq,
                selector: pas,
                ..
            } = &p.value
            {
                if paq.ne(qualifier) {
                    // p is not an alias because its qualifier does not match
                    continue;
                }
                match pas.as_ref() {
                    PathSelector::Index(i) => {
                        // paq[i] might alias an element in qualifier[index]
                        let indices_are_equal = index.equals(i.clone());
                        match indices_are_equal.as_bool_if_known() {
                            Some(true) => {
                                // p is known to be an alias of path, so just update it
                                self.strong_update_value_at(p.clone(), value.clone());
                            }
                            Some(false) => {
                                // p is known not to be an alias of path
                                continue;
                            }
                            None => {
                                // p might be an alias of path, so weaken its value by making it
                                // conditional on index == i
                                let conditional_value = indices_are_equal
                                    .conditional_expression(value.clone(), v.clone());
                                //debug!("conditional_value {:?}", conditional_value);
                                self.strong_update_value_at(p.clone(), conditional_value);
                            }
                        }
                    }
                    PathSelector::ConstantIndex { .. }
                    | PathSelector::ConstantSlice { .. }
                    | PathSelector::Slice(..) => {
                        let weakened_value = v.join(value.clone());
                        self.strong_update_value_at(p.clone(), weakened_value);
                    }
                    _ => {}
                }
            }
        }
    }

    /// If the path contains an abstract value that was constructed with a conditional, the path is
    /// concretized into two paths where the abstract value is replaced by the consequent
    /// and alternate, respectively. These paths can then be weakly updated to reflect the
    /// lack of precise knowledge at compile time.
    #[logfn_inputs(TRACE)]
    pub fn try_to_split(
        &mut self,
        path: &Rc<Path>,
    ) -> Option<(Rc<AbstractValue>, Rc<Path>, Rc<Path>)> {
        match &path.value {
            PathEnum::Computed { value } => {
                if let Expression::ConditionalExpression {
                    condition,
                    consequent,
                    alternate,
                } = &value.expression
                {
                    return Some((
                        condition.clone(),
                        Path::get_as_path(consequent.refine_with(condition, 0)),
                        Path::get_as_path(alternate.refine_with(&condition.logical_not(), 0)),
                    ));
                }
                None
            }
            PathEnum::QualifiedPath {
                ref qualifier,
                ref selector,
                ..
            } => {
                if let Some((condition, true_path, false_path)) = self.try_to_split(qualifier) {
                    let true_path =
                        if let Some(deref_true_path) = Path::try_to_dereference(&true_path, self) {
                            if *selector.as_ref() == PathSelector::Deref {
                                // deref_true_path is now the canonical version of true_path
                                deref_true_path
                            } else {
                                Path::new_qualified(true_path, selector.clone())
                            }
                        } else {
                            Path::new_qualified(true_path, selector.clone())
                        };

                    let false_path = if let Some(deref_false_path) =
                        Path::try_to_dereference(&false_path, self)
                    {
                        if *selector.as_ref() == PathSelector::Deref {
                            // deref_false_path is now the canonical version of false_path
                            deref_false_path
                        } else {
                            Path::new_qualified(false_path, selector.clone())
                        }
                    } else {
                        Path::new_qualified(false_path, selector.clone())
                    };

                    Some((condition, true_path, false_path))
                } else {
                    self.try_to_split_selector(qualifier, selector)
                }
            }
            _ => None,
        }
    }

    /// If the path selector contains an abstract value that was constructed with a conditional, the path is
    /// concretized into two paths where the abstract value is replaced by the consequent
    /// and alternate, respectively. These paths can then be weakly updated to reflect the
    /// lack of precise knowledge at compile time.
    #[logfn_inputs(TRACE)]
    fn try_to_split_selector(
        &mut self,
        qualifier: &Rc<Path>,
        selector: &Rc<PathSelector>,
    ) -> Option<(Rc<AbstractValue>, Rc<Path>, Rc<Path>)> {
        match selector.as_ref() {
            PathSelector::Index(value) => {
                if let Expression::ConditionalExpression {
                    condition,
                    consequent,
                    alternate,
                } = &value.expression
                {
                    return Some((
                        condition.clone(),
                        Path::new_index(qualifier.clone(), consequent.clone()),
                        Path::new_index(qualifier.clone(), alternate.clone()),
                    ));
                }
                None
            }
            PathSelector::Slice(value) => {
                if let Expression::ConditionalExpression {
                    condition,
                    consequent,
                    alternate,
                } = &value.expression
                {
                    return Some((
                        condition.clone(),
                        Path::new_slice(qualifier.clone(), consequent.clone()),
                        Path::new_slice(qualifier.clone(), alternate.clone()),
                    ));
                }
                None
            }
            _ => None,
        }
    }

    /// Returns an environment with a path for every entry in self and other and an associated
    /// value that is condition.conditional_expression(self.value_at(path), other.value_at(path))
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn conditional_join(
        &self,
        other: Environment,
        condition: &Rc<AbstractValue>,
        other_condition: &Rc<AbstractValue>,
    ) -> Environment {
        let (assumed_aliases, guarded_aliases) =
            self.conditional_join_aliases(&other, condition, other_condition);
        let mut result = self.join_or_widen(other, |x, y, _p| {
            if let (Expression::CompileTimeConstant(v1), Expression::CompileTimeConstant(v2)) =
                (&x.expression, &y.expression)
            {
                match (v1, v2) {
                    (ConstantDomain::True, ConstantDomain::False) => {
                        return condition.clone();
                    }
                    (ConstantDomain::False, ConstantDomain::True) => {
                        return other_condition.clone();
                    }
                    _ => (),
                }
            }
            condition.conditional_expression(x.clone(), y.clone())
        });
        result.assumed_aliases = assumed_aliases;
        result.guarded_aliases = guarded_aliases;
        result
    }

    /// Returns an environment with a path for every entry in self and other and an associated
    /// value that is the join of self.value_at(path) and other.value_at(path)
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn join(&self, other: Environment) -> Environment {
        self.join_or_widen(other, |x, y, p| {
            if let Some(val) = x.get_widened_subexpression(p) {
                return val;
            }
            if let Some(val) = y.get_widened_subexpression(p) {
                return val;
            }
            x.join(y.clone())
        })
    }

    /// Returns an environment with a path for every entry in self and other and an associated
    /// value that is the widen of self.value_at(path) and other.value_at(path)
    #[logfn_inputs(TRACE)]
    #[must_use]
    pub fn widen(&self, other: Environment) -> Environment {
        self.join_or_widen(other, |x, y, p| {
            if let Some(val) = x.get_widened_subexpression(p) {
                return val;
            }
            if let Some(val) = y.get_widened_subexpression(p) {
                return val;
            }
            if let (
                Expression::WidenedJoin { path: p1, .. },
                Expression::WidenedJoin { path: p2, .. },
            ) = (&x.expression, &y.expression)
            {
                if p1.eq(p2) || p1.eq(p) {
                    return x.clone();
                } else if p2.eq(p) {
                    return y.clone();
                }
            }
            x.join(y.clone()).widen(p)
        })
    }

    /// Returns a set of paths that do not have identical associated values in both self and other.
    #[logfn_inputs(TRACE)]
    pub fn get_loop_variants(&self, other: &Environment) -> HashSet<Rc<Path>> {
        let mut loop_variants: HashSet<Rc<Path>> = HashSet::new();
        let value_map1 = &self.value_map;
        let value_map2 = &other.value_map;
        for (path, val1) in value_map1.iter() {
            let p = path.clone();
            match value_map2.get(path) {
                Some(val2) => {
                    if !val1.eq(val2) {
                        loop_variants.insert(p);
                    }
                }
                None => {
                    loop_variants.insert(p);
                }
            }
        }
        for (path, _) in value_map2.iter() {
            if !value_map1.contains_key(path) {
                loop_variants.insert(path.clone());
            }
        }
        loop_variants
    }

    /// Returns an environment with a path for every entry in self and other and an associated
    /// value that is the join or widen of self.value_at(path) and other.value_at(path).
    #[logfn(TRACE)]
    fn join_or_widen<F>(&self, other: Environment, join_or_widen: F) -> Environment
    where
        F: Fn(&Rc<AbstractValue>, &Rc<AbstractValue>, &Rc<Path>) -> Rc<AbstractValue>,
    {
        let value_map1 = &self.value_map;
        let value_map2 = &other.value_map;
        let assumed_aliases = self
            .assumed_aliases
            .intersection(&other.assumed_aliases)
            .cloned()
            .collect();
        let mut guarded_aliases = HashMap::new();
        let mut relationships = self.alias_relationships();
        relationships.extend(other.alias_relationships());
        for relationship in relationships {
            if let (Some(left), Some(right)) = (
                self.alias_condition(&relationship),
                other.alias_condition(&relationship),
            ) {
                let condition = left.and(right);
                if condition.as_bool_if_known() == Some(true) {
                    continue;
                }
                if condition.as_bool_if_known() != Some(false) {
                    guarded_aliases.insert(relationship, condition);
                }
            }
        }
        let mut value_map: HashTrieMap<Rc<Path>, Rc<AbstractValue>> = value_map1.clone();
        for (path, val2) in value_map2.iter() {
            let p = path.clone();
            match value_map1.get(path) {
                Some(val1) => {
                    value_map.insert_mut(p, join_or_widen(val1, val2, path));
                }
                None => {
                    if !path.is_rooted_by_parameter() || val2.is_unit() {
                        // joining bottom and val2
                        // The bottom value corresponds to dead (impossible) code, so the join collapses.
                        value_map.insert_mut(p, val2.clone());
                    } else {
                        let val1 = AbstractValue::make_initial_parameter_value(
                            val2.expression.infer_type(),
                            path.clone(),
                        );
                        value_map.insert_mut(p, join_or_widen(&val1, val2, path));
                    };
                }
            }
        }
        Environment {
            value_map,
            entry_condition: abstract_value::TRUE.into(),
            assumed_aliases,
            guarded_aliases,
            exit_conditions: HashTrieMap::default(),
        }
    }

    fn alias_relationships(&self) -> HashSet<(Rc<Path>, Rc<Path>)> {
        self.assumed_aliases
            .iter()
            .cloned()
            .chain(self.guarded_aliases.keys().cloned())
            .collect()
    }

    fn alias_condition(&self, relationship: &(Rc<Path>, Rc<Path>)) -> Option<Rc<AbstractValue>> {
        if self.assumed_aliases.contains(relationship) {
            Some(Rc::new(abstract_value::TRUE))
        } else {
            self.guarded_aliases.get(relationship).cloned()
        }
    }

    fn conditional_join_aliases(
        &self,
        other: &Environment,
        condition: &Rc<AbstractValue>,
        other_condition: &Rc<AbstractValue>,
    ) -> (
        HashSet<(Rc<Path>, Rc<Path>)>,
        HashMap<(Rc<Path>, Rc<Path>), Rc<AbstractValue>>,
    ) {
        let mut assumed_aliases = HashSet::new();
        let mut guarded_aliases = HashMap::new();
        let mut relationships = self.alias_relationships();
        relationships.extend(other.alias_relationships());
        for relationship in relationships {
            if self.assumed_aliases.contains(&relationship)
                && other.assumed_aliases.contains(&relationship)
            {
                assumed_aliases.insert(relationship);
                continue;
            }
            let left = self
                .alias_condition(&relationship)
                .unwrap_or_else(|| Rc::new(abstract_value::FALSE));
            let right = other
                .alias_condition(&relationship)
                .unwrap_or_else(|| Rc::new(abstract_value::FALSE));
            let joined_condition = condition.and(left).or(other_condition.and(right));
            match joined_condition.as_bool_if_known() {
                Some(true) => {
                    assumed_aliases.insert(relationship);
                }
                Some(false) => {}
                None => {
                    guarded_aliases.insert(relationship, joined_condition);
                }
            }
        }
        (assumed_aliases, guarded_aliases)
    }

    /// Returns true if for every path, self.value_at(path).subset(other.value_at(path))
    #[logfn_inputs(TRACE)]
    pub fn subset(&self, other: &Environment) -> bool {
        let value_map1 = &self.value_map;
        let value_map2 = &other.value_map;
        for (path, val1) in value_map1.iter().filter(|(_, v)| !v.is_bottom()) {
            if let Some(val2) = value_map2.get(path) {
                if !(val1.subset(val2)) {
                    trace!("self at {:?} is {:?} other is {:?}", path, val1, val2);
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::Environment;
    use crate::abstract_value::{AbstractValue, AbstractValueTrait};
    use crate::expression::ExpressionType;
    use crate::path::{Path, PathRoot, PathSelector};
    use std::rc::Rc;

    fn computed_model_field(index_ordinal: usize) -> (Rc<Path>, Rc<AbstractValue>) {
        let collection = Path::new_local(1, 0);
        let index = AbstractValue::make_typed_unknown(
            ExpressionType::Usize,
            Path::new_local(index_ordinal, 0),
        );
        let element = Path::new_index(collection, index.clone());
        (
            Path::new_model_field(element, Rc::from("read_count")),
            index,
        )
    }

    #[test]
    fn aggregate_copy_rekeys_alias_endpoints() {
        let clone = Path::new_local(2, 0);
        let seed = Path::new_parameter(1);
        let moved_seed = Path::new_result();
        let mut environment = Environment::default();
        environment.assume_alias(clone.clone(), seed.clone());

        environment.copy_aliases_rooted_by(&moved_seed, &seed, false);

        assert!(environment.assumed_aliases.contains(&(clone, moved_seed)));
    }

    #[test]
    fn aggregate_move_removes_aliases_rooted_in_the_moved_from_place() {
        let moved_from = Path::new_local(2, 0);
        let external = Path::new_parameter(1);
        let destination = Path::new_result();
        let mut environment = Environment::default();
        environment.assume_alias(moved_from.clone(), external.clone());

        environment.copy_aliases_rooted_by(&destination, &moved_from, true);

        assert!(environment
            .assumed_aliases
            .contains(&(destination, external)));
        assert!(!environment
            .assumed_aliases
            .iter()
            .any(|(alias, source)| alias.is_rooted_by(&moved_from)
                || source.is_rooted_by(&moved_from)));
    }

    #[test]
    fn must_alias_canonicalization_is_independent_of_insertion_order() {
        let alias = Path::new_local(1, 0);
        let nested_alias = Path::new_qualified(alias.clone(), Rc::new(PathSelector::Field(0)));
        let shallow_source = Path::new_local(2, 0);
        let nested_source = Path::new_local(3, 0);
        let queried = Path::new_qualified(nested_alias.clone(), Rc::new(PathSelector::Field(1)));
        let expected = Path::new_qualified(nested_source.clone(), Rc::new(PathSelector::Field(1)));

        for relationships in [
            [
                (alias.clone(), shallow_source.clone()),
                (nested_alias.clone(), nested_source.clone()),
            ],
            [
                (nested_alias.clone(), nested_source.clone()),
                (alias.clone(), shallow_source.clone()),
            ],
        ] {
            let mut environment = Environment::default();
            for (alias, source) in relationships {
                environment.assume_alias(alias, source);
            }
            assert!(environment.paths_must_alias(&queried, &expected));
        }
    }

    #[test]
    fn must_alias_canonicalization_ignores_expanding_alias() {
        let alias = Path::new_local(1, 0);
        let expanding_source = Path::new_qualified(alias.clone(), Rc::new(PathSelector::Field(0)));
        let mut environment = Environment::default();
        environment.assume_alias(alias.clone(), expanding_source);

        assert!(environment.paths_must_alias(&alias, &alias));
    }

    #[test]
    fn computed_index_model_field_lookup_handles_alias_outcomes() {
        let zero: Rc<AbstractValue> = Rc::new(0_u128.into());
        let one: Rc<AbstractValue> = Rc::new(1_u128.into());
        let two: Rc<AbstractValue> = Rc::new(2_u128.into());
        let (first_path, first_index) = computed_model_field(2);
        let (second_path, second_index) = computed_model_field(3);
        let (query_path, query_index) = computed_model_field(4);

        let mut exact = Environment::default();
        exact.strong_update_value_at(first_path.clone(), one.clone());
        assert_eq!(
            exact.value_at_computed_index_model_field(&first_path, zero.clone()),
            Some(one.clone())
        );

        let mut equal = exact.clone();
        equal.entry_condition = first_index.equals(query_index.clone());
        assert_eq!(
            equal.value_at_computed_index_model_field(&query_path, zero.clone()),
            Some(one.clone())
        );

        let mut distinct = exact.clone();
        distinct.entry_condition = first_index.not_equals(query_index.clone());
        assert_eq!(
            distinct.value_at_computed_index_model_field(&query_path, zero.clone()),
            None
        );

        let unknown = exact.value_at_computed_index_model_field(&query_path, zero.clone());
        assert_ne!(unknown, None);
        assert_ne!(unknown, Some(zero.clone()));
        assert_ne!(unknown, Some(one.clone()));

        let mut multiple = exact;
        multiple.strong_update_value_at(second_path, two.clone());
        multiple.entry_condition = first_index
            .equals(query_index.clone())
            .and(second_index.equals(query_index));
        let multiple_value = multiple
            .value_at_computed_index_model_field(&query_path, zero)
            .unwrap();
        assert!(multiple_value == one.join(two.clone()) || multiple_value == two.join(one));
    }

    #[test]
    fn model_field_path_canonicalization_rewrites_aliased_prefix() {
        let source = Path::new_local(1, 0);
        let alias = Path::new_local(2, 0);
        let source_field =
            Path::new_qualified(source.clone(), Rc::new(crate::path::PathSelector::Field(3)));
        let alias_field =
            Path::new_qualified(alias.clone(), Rc::new(crate::path::PathSelector::Field(3)));
        let source_model = Path::new_model_field(source_field, Rc::from("write_held"));
        let alias_model = Path::new_model_field(alias_field, Rc::from("write_held"));

        let mut environment = Environment::default();
        environment.assume_alias(alias, source);

        assert_eq!(
            environment.canonicalize_model_field_path(alias_model),
            source_model
        );
    }

    #[test]
    fn model_field_path_canonicalization_folds_transferred_pointer_chain() {
        let result = Path::new_local(1, 0);
        let guard_projection = Path::new_local(2, 0);
        let receiver = Path::new_parameter(1);
        let dereferenced_result = Path::new_qualified(result.clone(), Rc::new(PathSelector::Deref));
        let model = Path::new_model_field(dereferenced_result, Rc::from("write_held"));
        let expected = Path::new_model_field(receiver.clone(), Rc::from("write_held"));

        let mut environment = Environment::default();
        environment.strong_update_value_at(
            result,
            AbstractValue::make_typed_unknown(
                ExpressionType::ThinPointer,
                guard_projection.clone(),
            ),
        );
        environment
            .strong_update_value_at(guard_projection, AbstractValue::make_reference(receiver));

        assert_eq!(environment.canonicalize_model_field_path(model), expected);
    }

    #[test]
    fn guarded_model_field_alias_lookup_is_conditional() {
        let source = Path::new_local(1, 0);
        let alias = Path::new_local(2, 0);
        let source_model = Path::new_model_field(source.clone(), Rc::from("write_held"));
        let alias_model = Path::new_model_field(alias.clone(), Rc::from("write_held"));
        let condition =
            AbstractValue::make_typed_unknown(ExpressionType::Bool, Path::new_local(3, 0));
        let zero: Rc<AbstractValue> = Rc::new(0_u128.into());
        let one: Rc<AbstractValue> = Rc::new(1_u128.into());

        let mut environment = Environment::default();
        environment.strong_update_value_at(source_model, one.clone());
        environment.assume_alias_if(alias, source, condition.clone());

        assert_eq!(
            environment.value_at_aliased_model_field(&alias_model, zero.clone()),
            Some(condition.conditional_expression(one, zero))
        );
    }

    #[test]
    fn model_field_path_canonicalization_ignores_expanding_alias() {
        let alias = Path::new_local(1, 0);
        let source =
            Path::new_qualified(alias.clone(), Rc::new(crate::path::PathSelector::Field(2)));
        let model = Path::new_model_field(alias.clone(), Rc::from("write_held"));

        let mut environment = Environment::default();
        environment.assume_alias(alias, source);

        assert_eq!(
            environment.canonicalize_model_field_path(model.clone()),
            model
        );
    }

    fn transmuted_pointer_model_field(pointer: Rc<AbstractValue>) -> Rc<Path> {
        let modulus: Rc<AbstractValue> = Rc::new((1_u128 << 64).into());
        let masked = AbstractValue::make_from(
            crate::expression::Expression::Rem {
                left: pointer,
                right: modulus,
            },
            1,
        );
        let pointer = AbstractValue::make_from(
            crate::expression::Expression::Transmute {
                operand: masked,
                target_type: ExpressionType::ThinPointer,
            },
            1,
        );
        let pointee = Path::new_qualified(
            Path::new_computed(pointer),
            Rc::new(crate::path::PathSelector::Deref),
        );
        let lock = Path::new_qualified(pointee, Rc::new(crate::path::PathSelector::Field(0)));
        Path::new_model_field(lock, Rc::from("write_held"))
    }

    #[test]
    fn model_field_lookup_strips_old_from_invariant_transmuted_pointer() {
        let parameter = Path::new_parameter(1);
        let current = AbstractValue::make_reference(parameter.clone());
        let initial =
            AbstractValue::make_initial_parameter_value(ExpressionType::U128, parameter.clone());
        let current_model = transmuted_pointer_model_field(current);
        let initial_model = transmuted_pointer_model_field(initial);

        let expected: Rc<AbstractValue> = Rc::new(1_u128.into());
        let mut environment = Environment::default();
        environment.strong_update_value_at(current_model.clone(), expected.clone());

        assert_eq!(
            environment.value_at_aliased_model_field(&initial_model, Rc::new(0_u128.into())),
            Some(expected)
        );
    }

    #[test]
    fn model_field_lookup_keeps_old_for_reassigned_transmuted_pointer() {
        let parameter = Path::new_parameter(1);
        let current = AbstractValue::make_reference(parameter.clone());
        let initial =
            AbstractValue::make_initial_parameter_value(ExpressionType::U128, parameter.clone());
        let current_model = transmuted_pointer_model_field(current);
        let initial_model = transmuted_pointer_model_field(initial);
        let mut environment = Environment::default();
        environment.strong_update_value_at(parameter, Rc::new(2_u128.into()));
        environment.strong_update_value_at(current_model, Rc::new(1_u128.into()));

        assert_eq!(
            environment.value_at_aliased_model_field(&initial_model, Rc::new(0_u128.into())),
            None
        );
    }

    #[test]
    fn model_field_lookup_unifies_refined_reference_projection() {
        use crate::path::PathSelector;

        let parameter = Path::new_parameter(1);
        let dereferenced = Path::new_qualified(parameter, Rc::new(PathSelector::Deref));
        let prefix = Path::new_qualified(dereferenced, Rc::new(PathSelector::Field(0)));
        let referenced_prefix = Path::new_computed(AbstractValue::make_reference(prefix.clone()));
        let mut projected = Path::new_qualified(referenced_prefix, Rc::new(PathSelector::Deref));
        for _ in 0..3 {
            projected = Path::new_qualified(projected, Rc::new(PathSelector::Field(0)));
        }
        let mut fully_projected = prefix;
        for _ in 0..3 {
            fully_projected = Path::new_qualified(fully_projected, Rc::new(PathSelector::Field(0)));
        }

        let ordinary_u128 =
            AbstractValue::make_typed_unknown(ExpressionType::U128, projected.clone());
        assert_eq!(
            ordinary_u128.normalize_reference_projections(),
            ordinary_u128
        );

        let write_pointer = AbstractValue::make_typed_unknown(ExpressionType::U128, projected);
        let write_model = transmuted_pointer_model_field(write_pointer);
        let canonical_write = Environment::default().canonicalize_model_field_path(write_model);
        let expected_pointer = AbstractValue::make_reference(fully_projected.clone());
        let expected_write = transmuted_pointer_model_field(expected_pointer);
        assert_eq!(canonical_write, expected_write);

        let read_model = transmuted_pointer_model_field(
            AbstractValue::make_initial_parameter_value(ExpressionType::U128, fully_projected),
        );
        let expected: Rc<AbstractValue> = Rc::new(1_u128.into());
        let mut environment = Environment::default();
        environment.strong_update_value_at(canonical_write, expected.clone());

        assert_eq!(
            environment.value_at_aliased_model_field(&read_model, Rc::new(0_u128.into())),
            Some(expected)
        );
    }
}
