pub mod image;

#[derive(Debug, Default, Clone)]
pub struct InferOutput {
    pub bounding_box: BoundingBox,
    pub score: f32,
    pub key_points: Vec<KeyPoint>
}

#[derive(Debug, Default, Clone)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[derive(Debug, Default, Clone)]
pub struct KeyPoint {
    pub x: f32,
    pub y: f32,
    pub visibility: f32,
}