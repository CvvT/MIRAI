use rwlock_annotation_detection::{DescriptorOwner, DescriptorTable, Metadata};

fn with_metadata_after_release<R>(
    owner: &DescriptorOwner,
    callback: impl FnOnce(&mut Metadata) -> R,
) -> R {
    let guard = owner.descriptor_table_mut();
    drop(guard);
    DescriptorTable::new().with_metadata_mut(callback)
}

fn main() {
    let owner = DescriptorOwner;
    with_metadata_after_release(&owner, |_metadata| owner.read());
}
