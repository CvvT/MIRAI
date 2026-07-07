use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SummaryProvenance {
    ReviewedContract,
    BodyDerived,
    Dummy,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LockLevel {
    Database,
    Cache,
    Disk,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ResourceId {
    receiver: u32,
    field: u32,
    level: LockLevel,
}

impl ResourceId {
    const fn new(receiver: u32, field: u32, level: LockLevel) -> Self {
        Self {
            receiver,
            field,
            level,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OrderEdge {
    before: LockLevel,
    after: LockLevel,
}

impl OrderEdge {
    const fn new(before: LockLevel, after: LockLevel) -> Self {
        Self { before, after }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LockContract {
    Acquire {
        provenance: SummaryProvenance,
        resource: ResourceId,
    },
    Release {
        provenance: SummaryProvenance,
        resource: ResourceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewedCallee {
    contract: LockContract,
    body_summary: LockContract,
}

impl LockContract {
    fn provenance(&self) -> SummaryProvenance {
        match self {
            Self::Acquire { provenance, .. } | Self::Release { provenance, .. } => *provenance,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CheckError {
    BodyDerivedSummaryRejected,
    DummySummaryRejected,
    MissingContract,
    DoubleLock(ResourceId),
    UnheldRelease(ResourceId),
    OrderViolation {
        held: ResourceId,
        acquired: ResourceId,
    },
}

#[derive(Default)]
struct LockWorld {
    held: BTreeSet<ResourceId>,
    order_edges: BTreeSet<OrderEdge>,
}

impl LockWorld {
    fn declare_edge(&mut self, before: LockLevel, after: LockLevel) {
        self.order_edges.insert(OrderEdge::new(before, after));
    }

    fn apply_contract(&mut self, contract: Option<&LockContract>) -> Result<(), CheckError> {
        let contract = contract.ok_or(CheckError::MissingContract)?;
        match contract.provenance() {
            SummaryProvenance::ReviewedContract => {}
            SummaryProvenance::BodyDerived => return Err(CheckError::BodyDerivedSummaryRejected),
            SummaryProvenance::Dummy => return Err(CheckError::DummySummaryRejected),
        }

        match contract {
            LockContract::Acquire { resource, .. } => self.acquire(*resource),
            LockContract::Release { resource, .. } => {
                if self.held.remove(resource) {
                    Ok(())
                } else {
                    Err(CheckError::UnheldRelease(*resource))
                }
            }
        }
    }

    fn apply_reviewed_callee(&mut self, callee: &ReviewedCallee) -> Result<(), CheckError> {
        self.apply_contract(Some(&callee.contract))
    }

    fn acquire(&mut self, acquired: ResourceId) -> Result<(), CheckError> {
        if self.held.contains(&acquired) {
            return Err(CheckError::DoubleLock(acquired));
        }

        for held in &self.held {
            if held.receiver != acquired.receiver || held.level == acquired.level {
                continue;
            }
            let edge = OrderEdge::new(held.level, acquired.level);
            if !self.order_edges.contains(&edge) {
                return Err(CheckError::OrderViolation {
                    held: *held,
                    acquired,
                });
            }
        }

        self.held.insert(acquired);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CheckError, LockContract, LockLevel, LockWorld, ResourceId, ReviewedCallee,
        SummaryProvenance,
    };

    const FIRST_DATABASE: ResourceId = ResourceId::new(0, 0, LockLevel::Database);
    const FIRST_CACHE: ResourceId = ResourceId::new(0, 1, LockLevel::Cache);
    const FIRST_DISK: ResourceId = ResourceId::new(0, 2, LockLevel::Disk);
    const SECOND_DISK: ResourceId = ResourceId::new(1, 2, LockLevel::Disk);

    fn reviewed_acquire(resource: ResourceId) -> LockContract {
        LockContract::Acquire {
            provenance: SummaryProvenance::ReviewedContract,
            resource,
        }
    }

    fn body_derived_acquire(resource: ResourceId) -> LockContract {
        LockContract::Acquire {
            provenance: SummaryProvenance::BodyDerived,
            resource,
        }
    }

    fn reviewed_release(resource: ResourceId) -> LockContract {
        LockContract::Release {
            provenance: SummaryProvenance::ReviewedContract,
            resource,
        }
    }

    fn ordered_world() -> LockWorld {
        let mut world = LockWorld::default();
        world.declare_edge(LockLevel::Database, LockLevel::Cache);
        world.declare_edge(LockLevel::Cache, LockLevel::Disk);
        world
    }

    fn ordered_world_with_database_to_disk() -> LockWorld {
        let mut world = ordered_world();
        world.declare_edge(LockLevel::Database, LockLevel::Disk);
        world
    }

    #[test]
    fn acquiring_from_empty_held_set_is_accepted() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_CACHE))),
            Ok(())
        );
    }

    #[test]
    fn direct_edge_is_accepted_from_reviewed_contracts() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DATABASE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_CACHE))),
            Ok(())
        );
    }

    #[test]
    fn pairwise_order_is_not_transitively_closed() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DATABASE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DISK))),
            Err(CheckError::OrderViolation {
                held: FIRST_DATABASE,
                acquired: FIRST_DISK,
            })
        );
    }

    #[test]
    fn explicit_database_to_disk_edge_allows_database_then_disk() {
        let mut world = ordered_world_with_database_to_disk();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DATABASE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DISK))),
            Ok(())
        );
    }

    #[test]
    fn reverse_order_without_direct_edge_is_rejected() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_CACHE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DATABASE))),
            Err(CheckError::OrderViolation {
                held: FIRST_CACHE,
                acquired: FIRST_DATABASE,
            })
        );
    }

    #[test]
    fn resources_are_receiver_relative() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DATABASE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(SECOND_DISK))),
            Ok(())
        );
    }

    #[test]
    fn held_state_is_removed_on_release() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DATABASE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_release(FIRST_DATABASE))),
            Ok(())
        );
        assert_eq!(
            world.apply_contract(Some(&reviewed_acquire(FIRST_DISK))),
            Ok(())
        );
    }

    #[test]
    fn releasing_unheld_resource_is_rejected() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&reviewed_release(FIRST_DATABASE))),
            Err(CheckError::UnheldRelease(FIRST_DATABASE))
        );
    }

    #[test]
    fn only_reviewed_contract_provenance_is_accepted() {
        let mut world = ordered_world();

        assert_eq!(
            world.apply_contract(Some(&body_derived_acquire(FIRST_DATABASE))),
            Err(CheckError::BodyDerivedSummaryRejected)
        );
        assert_eq!(
            world.apply_contract(Some(&LockContract::Acquire {
                provenance: SummaryProvenance::Dummy,
                resource: FIRST_DATABASE,
            })),
            Err(CheckError::DummySummaryRejected)
        );
        assert_eq!(world.apply_contract(None), Err(CheckError::MissingContract));
    }

    #[test]
    fn reviewed_contract_is_used_instead_of_body_summary() {
        let mut world = ordered_world();
        let callee = ReviewedCallee {
            contract: reviewed_acquire(FIRST_DATABASE),
            // If the boundary read a body-derived summary, this would either be
            // rejected for provenance or acquire a different resource.
            body_summary: body_derived_acquire(FIRST_DISK),
        };

        assert_eq!(world.apply_reviewed_callee(&callee), Ok(()));
        assert!(world.held.contains(&FIRST_DATABASE));
        assert!(!world.held.contains(&FIRST_DISK));
    }
}
