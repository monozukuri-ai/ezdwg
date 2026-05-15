pub mod config;
pub mod error;
pub mod handle_allocator;
pub mod ir;
pub mod object_graph;
pub mod r2000;

pub use config::WriterConfig;
pub use handle_allocator::HandleAllocator;
pub use ir::{
    ArcEntity, CircleEntity, CommonEntityProps, LayerDef, LineEntity, LwPolylineEntity,
    MTextEntity, PointEntity, RayEntity, TextEntity, WriterDocument, WriterEntity, WriterMetadata,
    XLineEntity,
};
