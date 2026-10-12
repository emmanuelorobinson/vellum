// A transaction is not what a store applies: only a validated commit from the op layer is.
use vellum_core::ops::Transaction;
use vellum_core::ports::DocStore;

fn sneak(store: &mut dyn DocStore, tx: Transaction) {
    store.apply(tx);
}

fn main() {}
