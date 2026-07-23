use rwlock_annotation_detection::{DescriptorOwner, Metadata};

fn with_metadata_maybe_locked<R>(
    owner: &DescriptorOwner,
    acquire: bool,
    callback: impl FnOnce(&mut Metadata) -> R,
) -> R {
    owner
        .descriptor_table_mut_if(acquire)
        .with_metadata_mut(callback)
}

fn main() {
    let owner = DescriptorOwner;
    with_metadata_maybe_locked(&owner, false, |_metadata| owner.read());
}
