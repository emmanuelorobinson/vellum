// Invariant 11: only the broker makes a permit. Its fields are private and it has no public constructor.
use vellum_core::permissions::Permit;
use vellum_core::ports::{Action, EventSeq, PermissionKind};
use vellum_types::Origin;

fn forge(action: Action, origin: Origin) -> Permit {
    Permit {
        action,
        kind: PermissionKind::ReadRepo,
        origin,
        record: EventSeq(1),
    }
}

fn main() {}
