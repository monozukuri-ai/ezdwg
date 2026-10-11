pub mod config;
pub mod error;
pub mod handle_allocator;
pub mod ir;
pub mod object_graph;
pub mod r2000;
pub mod validation;

pub use config::WriterConfig;
pub use handle_allocator::HandleAllocator;
pub use ir::{
    ArcEntity, AttributeEntity, BlockDef, CircleEntity, CommonEntityProps, EllipseEntity,
    Face3dEntity, InsertEntity, LayerDef, LineEntity, LwPolylineEntity, MTextEntity,
    PlanarFaceEntity, PointEntity, RayEntity, TextEntity, WriterDocument, WriterEntity,
    WriterMetadata, XLineEntity,
};
