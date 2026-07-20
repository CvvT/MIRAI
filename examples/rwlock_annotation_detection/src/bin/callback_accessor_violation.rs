use rwlock_annotation_detection::ModeledRwLock;

struct Metadata {
    descriptor_lock: ModeledRwLock<()>,
}

impl Metadata {
    fn descriptor_lock(&self) -> &ModeledRwLock<()> {
        &self.descriptor_lock
    }
}

fn with_metadata_mut<F>(metadata: &Metadata, callback: F)
where
    F: FnOnce(&ModeledRwLock<()>),
{
    let lock = metadata.descriptor_lock();
    let writer = lock.write();
    callback(lock);
    drop(writer);
}

fn main() {
    let metadata = Metadata {
        descriptor_lock: ModeledRwLock::new(()),
    };
    with_metadata_mut(&metadata, |lock| {
        let reader = lock.read();
        std::hint::black_box(reader);
    });
}
