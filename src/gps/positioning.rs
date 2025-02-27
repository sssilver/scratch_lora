use defmt::Format;

#[derive(Debug, Clone, PartialEq, Format)]
pub struct Positioning {
    pub latitude: f32,
    pub longitude: f32,
}
