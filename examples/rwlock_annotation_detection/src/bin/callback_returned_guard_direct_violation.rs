use rwlock_annotation_detection::DescriptorOwner;

fn main() {
    let owner = DescriptorOwner;
    owner
        .descriptor_table_mut()
        .with_metadata_mut(|_metadata| owner.read());
}
