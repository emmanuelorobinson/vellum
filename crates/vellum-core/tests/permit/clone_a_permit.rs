// "Allow once" is once: a permit cannot be copied to perform its action twice.
use vellum_core::permissions::Permit;

fn twice(permit: &Permit) -> (Permit, Permit) {
    (permit.clone(), permit.clone())
}

fn main() {}
