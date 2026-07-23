use rwlock_annotation_detection::{DescriptorOwner, Metadata};

fn with_metadata_mut<R>(owner: &DescriptorOwner, callback: impl FnOnce(&mut Metadata) -> R) -> R {
    // The callback target is the dereferenced table, so the sibling guard owner is not an
    // argument of DescriptorTable::with_metadata_mut. Forwarding through this boundary matches
    // LiteBox's with_socket_options_mut and is required to reproduce the state loss.
    owner.descriptor_table_mut().with_metadata_mut(callback)
}

fn main() {
    let owner = DescriptorOwner;
    with_metadata_mut(&owner, |_metadata| owner.read());
}
