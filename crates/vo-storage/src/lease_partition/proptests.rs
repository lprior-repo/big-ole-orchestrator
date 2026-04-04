// TODO: Re-enable after proptest 1.11.0 macro regression is resolved.
// The `proptest!` macro misparses `fn` definitions when `use super::*;`
// imports conflicting names from the lease_partition parent module.
