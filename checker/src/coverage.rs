// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GapId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyStatus {
    NotViolated,
    PossibleViolation,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "reason")]
pub enum CoverageStatus {
    Complete,
    Incomplete(CoverageGapKind),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageGapKind {
    BaselineDiagnosticSuppression,
    BodyTimeout,
    CallbackArgumentDependencyLoss,
    CrateTimeout,
    DeriveGenerated,
    ElementTrackingBound,
    ExistentialCheckUndecided,
    ExistentialEncodingIncomplete,
    IncompleteAnalysis,
    IncompleteSummaryEffectLoss,
    PathLengthBound,
    PreconditionBottom,
    PreconditionTop,
    RecursionBound,
    RecursiveReentrySuppression,
    ReplayValidationUndecided,
    SelfSpecializedCallbackSuppression,
    SkippedRoot,
    SubsequentObligationDiscoveryStopped,
    UnknownOffsetSafety,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingTier {
    MustViolation,
    AbstractCounterexample,
    SolverRefutation,
    CoverageGap,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ReplayValidation {
    NotApplicable,
    Undecided,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CoverageRecord {
    site: String,
    property_status: PropertyStatus,
    coverage_status: CoverageStatus,
    tier: FindingTier,
    witness: Option<String>,
    replay_validation: ReplayValidation,
    gap_id: GapId,
    def_id: String,
    summary_key: String,
}

impl CoverageRecord {
    pub fn gap(site: String, def_id: String, summary_key: String, kind: CoverageGapKind) -> Self {
        let reason = serde_json::to_value(kind)
            .expect("coverage gap kind must serialize")
            .as_str()
            .expect("coverage gap kind must serialize as a string")
            .to_owned();
        Self {
            site,
            property_status: PropertyStatus::Unknown,
            coverage_status: CoverageStatus::Incomplete(kind),
            tier: FindingTier::CoverageGap,
            witness: None,
            replay_validation: ReplayValidation::NotApplicable,
            gap_id: GapId(format!("{reason}:{summary_key}")),
            def_id,
            summary_key,
        }
    }

    pub fn abstract_counterexample(
        site: String,
        def_id: String,
        summary_key: String,
        witness: String,
    ) -> Self {
        Self {
            site,
            property_status: PropertyStatus::PossibleViolation,
            coverage_status: CoverageStatus::Complete,
            tier: FindingTier::AbstractCounterexample,
            witness: Some(witness),
            replay_validation: ReplayValidation::Undecided,
            gap_id: GapId(format!("abstract-counterexample:{summary_key}")),
            def_id,
            summary_key,
        }
    }

    pub fn solver_refutation(site: String, def_id: String, summary_key: String) -> Self {
        Self {
            site,
            property_status: PropertyStatus::NotViolated,
            coverage_status: CoverageStatus::Complete,
            tier: FindingTier::SolverRefutation,
            witness: None,
            replay_validation: ReplayValidation::NotApplicable,
            gap_id: GapId(format!("existential-unsat:{summary_key}")),
            def_id,
            summary_key,
        }
    }

    pub fn sort_key(&self) -> (&str, &str, &str) {
        (&self.summary_key, &self.site, &self.gap_id.0)
    }
}

#[derive(Serialize)]
pub struct CoverageManifest<'a> {
    pub crate_name: &'a str,
    pub clean: bool,
    pub outcome: CoverageOutcome,
    pub envelope_sealed: bool,
    pub analyzed_bodies: usize,
    pub finding_count: usize,
    pub gap_counts: &'a BTreeMap<CoverageGapKind, usize>,
    pub records: &'a [CoverageRecord],
}

pub fn count_gaps(records: &[CoverageRecord]) -> BTreeMap<CoverageGapKind, usize> {
    let mut counts = BTreeMap::new();
    for record in records {
        if let CoverageStatus::Incomplete(kind) = record.coverage_status {
            *counts.entry(kind).or_default() += 1;
        }
    }
    counts
}

pub fn envelope_is_sealed(
    completed_enumeration: bool,
    accounted_roots: usize,
    declared_roots: usize,
) -> bool {
    completed_enumeration && accounted_roots == declared_roots
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageOutcome {
    Clean,
    CoverageNotEstablished,
    Incomplete,
}

impl CoverageOutcome {
    pub fn from_ledger(
        records: &[CoverageRecord],
        analyzed_bodies: usize,
        finding_count: usize,
        envelope_sealed: bool,
    ) -> Self {
        if !envelope_sealed || analyzed_bodies == 0 {
            Self::CoverageNotEstablished
        } else if finding_count > 0
            || records.iter().any(|record| {
                !matches!(record.coverage_status, CoverageStatus::Complete)
                    || !matches!(record.property_status, PropertyStatus::NotViolated)
                    || !matches!(record.replay_validation, ReplayValidation::NotApplicable)
            })
        {
            Self::Incomplete
        } else {
            Self::Clean
        }
    }

    pub fn is_clean(self) -> bool {
        self == Self::Clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_manifest_is_machine_readable() {
        let record = CoverageRecord::gap(
            "src/lib.rs:1:1".to_owned(),
            "DefId(0:1)".to_owned(),
            "test".to_owned(),
            CoverageGapKind::IncompleteAnalysis,
        );
        let records = [record];
        let gap_counts = count_gaps(&records);
        let outcome = CoverageOutcome::from_ledger(&records, 1, 0, true);
        let json = serde_json::to_string(&CoverageManifest {
            crate_name: "smoke",
            clean: outcome.is_clean(),
            outcome,
            envelope_sealed: true,
            analyzed_bodies: 1,
            finding_count: 0,
            gap_counts: &gap_counts,
            records: &records,
        })
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["crate_name"], "smoke");
        assert_eq!(value["clean"], false);
        assert_eq!(value["outcome"], "incomplete");
        assert_eq!(value["gap_counts"]["incomplete_analysis"], 1);
        assert_eq!(value["records"][0]["property_status"], "unknown");
        assert_eq!(
            value["records"][0]["coverage_status"]["reason"],
            "incomplete_analysis"
        );
        assert_eq!(value["records"][0]["tier"], "coverage_gap");
    }

    #[test]
    fn empty_manifest_does_not_claim_clean_coverage() {
        let outcome = CoverageOutcome::from_ledger(&[], 0, 0, true);
        assert_eq!(outcome, CoverageOutcome::CoverageNotEstablished);
        assert!(!outcome.is_clean());
    }

    #[test]
    fn sealed_gap_free_analyzed_envelope_is_clean() {
        let outcome = CoverageOutcome::from_ledger(&[], 1, 0, true);
        assert_eq!(outcome, CoverageOutcome::Clean);
        assert!(outcome.is_clean());
    }

    #[test]
    fn findings_prevent_clean_coverage() {
        let outcome = CoverageOutcome::from_ledger(&[], 1, 1, true);
        assert_eq!(outcome, CoverageOutcome::Incomplete);
        assert!(!outcome.is_clean());
    }

    #[test]
    fn complete_solver_refutation_preserves_clean_coverage() {
        let records = [CoverageRecord::solver_refutation(
            "src/lib.rs:1:1".to_owned(),
            "DefId(0:1)".to_owned(),
            "test".to_owned(),
        )];
        let outcome = CoverageOutcome::from_ledger(&records, 1, 0, true);
        assert_eq!(outcome, CoverageOutcome::Clean);
        assert!(outcome.is_clean());
        let value = serde_json::to_value(&records[0]).unwrap();
        assert_eq!(value["property_status"], "not_violated");
        assert_eq!(value["coverage_status"]["status"], "complete");
        assert_eq!(value["tier"], "solver_refutation");
        assert_eq!(value["witness"], serde_json::Value::Null);
    }

    #[test]
    fn abstract_models_prevent_clean_coverage() {
        let abstract_record = CoverageRecord::abstract_counterexample(
            "src/lib.rs:1:1".to_owned(),
            "DefId(0:1)".to_owned(),
            "test".to_owned(),
            "x -> 1".to_owned(),
        );
        let outcome = CoverageOutcome::from_ledger(&[abstract_record], 1, 0, true);
        assert_eq!(outcome, CoverageOutcome::Incomplete);
        assert!(!outcome.is_clean());
    }

    #[test]
    fn unresolved_replay_independently_prevents_clean_coverage() {
        let mut record = CoverageRecord::solver_refutation(
            "src/lib.rs:1:1".to_owned(),
            "DefId(0:1)".to_owned(),
            "test".to_owned(),
        );
        record.replay_validation = ReplayValidation::Undecided;

        let outcome = CoverageOutcome::from_ledger(&[record], 1, 0, true);
        assert_eq!(outcome, CoverageOutcome::Incomplete);
        assert!(!outcome.is_clean());
    }

    #[test]
    fn unaccounted_root_prevents_sealed_envelope() {
        assert!(!envelope_is_sealed(true, 1, 2));
        assert!(!envelope_is_sealed(false, 2, 2));
        assert!(envelope_is_sealed(true, 2, 2));
    }
}
