use super::queue::ClassifiedWrite;
use super::write_class::WriteClass;
use vo_types::events::EventEnvelope;

#[derive(Debug, Clone)]
pub enum AppendEntry {
    ControlPlane(ControlPlaneWrite),
    Projection(ProjectionWrite),
    Blob(BlobWrite),
}

impl ClassifiedWrite for AppendEntry {
    fn write_class(&self) -> WriteClass {
        match self {
            Self::ControlPlane(w) => w.write_class(),
            Self::Projection(w) => w.write_class(),
            Self::Blob(w) => w.write_class(),
        }
    }

    fn size_bytes(&self) -> u64 {
        match self {
            Self::ControlPlane(w) => w.size_bytes(),
            Self::Projection(w) => w.size_bytes(),
            Self::Blob(w) => w.size_bytes(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ControlPlaneWrite {
    pub event: EventEnvelope,
    size_bytes: u64,
}

impl ControlPlaneWrite {
    #[must_use]
    pub const fn new(event: EventEnvelope, size_bytes: u64) -> Self {
        Self { event, size_bytes }
    }
}

impl ClassifiedWrite for ControlPlaneWrite {
    fn write_class(&self) -> WriteClass {
        WriteClass::CriticalControlPlane
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionWrite {
    pub projection_id: String,
    size_bytes: u64,
}

impl ProjectionWrite {
    #[must_use]
    pub const fn new(projection_id: String, size_bytes: u64) -> Self {
        Self {
            projection_id,
            size_bytes,
        }
    }
}

impl ClassifiedWrite for ProjectionWrite {
    fn write_class(&self) -> WriteClass {
        WriteClass::OperatorProjection
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

#[derive(Debug, Clone)]
pub struct BlobWrite {
    pub blob_id: String,
    size_bytes: u64,
    class: WriteClass,
}

impl BlobWrite {
    #[must_use]
    pub const fn bulk(blob_id: String, size_bytes: u64) -> Self {
        Self {
            blob_id,
            size_bytes,
            class: WriteClass::BulkBlob,
        }
    }
}

impl ClassifiedWrite for BlobWrite {
    fn write_class(&self) -> WriteClass {
        self.class
    }

    fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}
