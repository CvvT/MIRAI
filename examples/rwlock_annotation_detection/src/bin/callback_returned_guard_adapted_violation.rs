use rwlock_annotation_detection::{DescriptorOwner, Metadata};

fn with_adapted_metadata_mut<R>(
    owner: &DescriptorOwner,
    callback: impl FnOnce(&mut Metadata) -> R,
) -> R {
    owner
        .descriptor_table_mut()
        .with_metadata_mut(|metadata| callback(metadata))
}

fn main() {
    let owner = DescriptorOwner;
    with_adapted_metadata_mut(&owner, |_metadata| owner.read());
}
